/** Current SELECT/OPTION state shared by DOM accessors and form submission. */
use super::{DomLevel, Page};
use crate::{
    document::ScriptChange,
    protocol::{EngineError, EngineResult},
};

impl Page<'_> {
    /** The nearest SELECT owns an option, including options below OPTGROUP. */
    pub fn option_owner(&mut self, node: usize) -> Option<usize> {
        let mut parent = self.parent(node);
        while let Some(id) = parent {
            if self.tag(id) == "select" {
                return Some(id);
            }
            parent = self.parent(id);
        }
        None
    }
    /** Ordered options never cross a nested SELECT boundary. */
    pub fn select_options(&mut self, node: usize) -> Vec<usize> {
        self.descendants(node)
            .into_iter()
            .filter(|id| self.tag(*id) == "option" && self.option_owner(*id) == Some(node))
            .collect()
    }
    /** Dirty script state wins; otherwise use the native current state, then detached markup. */
    pub fn is_selected(&mut self, node: usize) -> bool {
        if let Some(selected) = self.state.selected_options.get(&node) {
            return *selected;
        }
        if let Some(selected) = self.projected(node).and_then(|n| n.selected) {
            return selected;
        }
        let marked = self
            .view(node)
            .is_some_and(|n| n.attributes.contains_key("selected"));
        let Some(owner) = self.option_owner(node) else {
            return marked;
        };
        if self
            .view(owner)
            .is_some_and(|n| n.attributes.contains_key("multiple"))
        {
            return marked;
        }
        let options = self.select_options(owner);
        let explicit = options
            .iter()
            .rev()
            .find(|id| {
                self.view(**id)
                    .is_some_and(|n| n.attributes.contains_key("selected"))
            })
            .copied();
        let selected = explicit.or_else(|| {
            self.single_display_row(owner)
                .then(|| options.into_iter().find(|id| !self.option_disabled(*id)))
                .flatten()
        });
        selected == Some(node)
    }
    /** Option values use normalized text only when the value attribute is absent. */
    pub fn option_value(&mut self, node: usize) -> String {
        self.view(node)
            .and_then(|n| n.attributes.get("value").cloned())
            .unwrap_or_else(|| {
                self.text_content(node)
                    .split_ascii_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
    }
    /** SELECT value always describes the first currently selected option. */
    pub fn selected_value(&mut self, node: usize) -> String {
        self.select_options(node)
            .into_iter()
            .find(|id| self.is_selected(*id))
            .map(|id| self.option_value(id))
            .unwrap_or_default()
    }
    /** Unmatched values deliberately leave the control without a selection. */
    pub fn select_value(&mut self, node: usize, value: &str) -> EngineResult<()> {
        let selected = self
            .select_options(node)
            .into_iter()
            .find(|id| self.option_value(*id) == value);
        if let Some(option) = selected {
            self.state.dirty_options.insert(option);
        }
        self.select_exact(node, &selected.into_iter().collect::<Vec<_>>())
    }
    /** An out-of-range index clears selection rather than choosing the first option. */
    pub fn select_index(&mut self, node: usize, index: i32) -> EngineResult<()> {
        let selected = usize::try_from(index)
            .ok()
            .and_then(|i| self.select_options(node).get(i).copied());
        if let Some(option) = selected {
            self.state.dirty_options.insert(option);
        }
        self.select_exact(node, &selected.into_iter().collect::<Vec<_>>())
    }
    /** Setting an option changes current selectedness without changing defaultSelected. */
    pub fn option_selected(&mut self, node: usize, selected: bool) -> EngineResult<()> {
        self.remember_selected(node, selected)?;
        self.state.dirty_options.insert(node);
        let Some(owner) = self.option_owner(node) else {
            return Ok(());
        };
        let multiple = self
            .view(owner)
            .is_some_and(|n| n.attributes.contains_key("multiple"));
        let choices = self
            .select_options(owner)
            .into_iter()
            .filter(|id| {
                (*id == node && selected)
                    || ((*id != node) && (multiple || !selected) && self.is_selected(*id))
            })
            .collect::<Vec<_>>();
        self.select_exact(owner, &choices)?;
        self.normalize_selection(owner)
    }
    /** Only changes to this SELECT's own option list trigger selectedness normalization. */
    pub fn changed_option_owner(&mut self, node: usize) -> Option<usize> {
        let owner = self.option_owner(node)?;
        let contains_option = self.tag(node) == "option"
            || self.descendants(node).into_iter().any(|child| {
                self.tag(child) == "option" && self.option_owner(child) == Some(owner)
            });
        contains_option.then_some(owner)
    }
    /** Option mutations restore the default choice; explicit SELECT value/index assignments do not. */
    pub fn normalize_selection(&mut self, owner: usize) -> EngineResult<()> {
        let multiple = self
            .view(owner)
            .is_some_and(|node| node.attributes.contains_key("multiple"));
        if multiple {
            return Ok(());
        }
        let options = self.select_options(owner);
        let mut selected: Vec<_> = options
            .iter()
            .copied()
            .filter(|id| self.is_selected(*id))
            .collect();
        if selected.is_empty() && self.single_display_row(owner) {
            selected.extend(options.into_iter().find(|id| !self.option_disabled(*id)));
        } else if selected.len() > 1 {
            selected = selected.last().copied().into_iter().collect();
        }
        self.select_exact(owner, &selected)
    }
    /** Only a single-row dropdown automatically chooses its first enabled option. */
    fn single_display_row(&mut self, owner: usize) -> bool {
        !self.view(owner).is_some_and(|node| {
            node.attributes.contains_key("multiple")
                || node
                    .attributes
                    .get("size")
                    .and_then(|size| size.parse::<usize>().ok())
                    .is_some_and(|size| size > 1)
        })
    }
    /** Bound local option overrides just like detached nodes; arbitrary IDs cannot allocate state. */
    fn remember_selected(&mut self, node: usize, selected: bool) -> EngineResult<()> {
        if self.tag(node) != "option" {
            return Err(EngineError::invalid("Selection requires an option."));
        }
        if !self.state.selected_options.contains_key(&node)
            && self.state.selected_options.len() >= crate::constants::VM_DETACHED_NODES
        {
            return Err(EngineError::limit(
                "option states",
                crate::constants::VM_DETACHED_NODES,
            ));
        }
        self.state.selected_options.insert(node, selected);
        Ok(())
    }
    /** Local state and the native change log carry the same exact option identities. */
    pub fn select_exact(&mut self, node: usize, selected: &[usize]) -> EngineResult<()> {
        self.touch();
        for option in self.select_options(node) {
            self.remember_selected(option, selected.contains(&option))?;
        }
        if self.is_projected(node) && self.environment.dom_level == DomLevel::Extended {
            self.record(ScriptChange::Selection {
                node,
                selected: selected.to_vec(),
            })?;
        }
        Ok(())
    }
    /** Reflect defaultSelected without overwriting a dirty current selection. */
    pub fn selected_attribute(&mut self, node: usize, selected: bool) -> EngineResult<()> {
        if !self.state.dirty_options.contains(&node) {
            self.option_selected(node, selected)?;
            self.state.dirty_options.remove(&node);
        } else if let Some(owner) = self.option_owner(node) {
            let selected = self
                .select_options(owner)
                .into_iter()
                .filter(|id| self.is_selected(*id))
                .collect::<Vec<_>>();
            self.select_exact(owner, &selected)?;
        }
        Ok(())
    }
    /** Successful options exclude disabled options and disabled OPTGROUP ancestors. */
    pub fn successful_options(&mut self, node: usize) -> Vec<String> {
        self.select_options(node)
            .into_iter()
            .filter(|id| self.is_selected(*id) && !self.option_disabled(*id))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|id| self.option_value(id))
            .collect()
    }
    /** Script selection may choose disabled options, but form submission must omit them. */
    fn option_disabled(&mut self, node: usize) -> bool {
        let mut current = Some(node);
        while let Some(id) = current {
            if self.tag(id) == "select" {
                break;
            }
            if self
                .view(id)
                .is_some_and(|n| n.attributes.contains_key("disabled"))
            {
                return true;
            }
            current = self.parent(id);
        }
        false
    }
}
