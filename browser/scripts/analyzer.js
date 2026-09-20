/**
 * Oya Browser Page Analyzer for Electron — same logic as the extension version,
 * but callable directly as analyzePage() without chrome.runtime messaging.
 */

(function () {
  'use strict';

  if (window.__acAnalyzerLoaded) return;
  window.__acAnalyzerLoaded = true;

  // Attribute used to tag indexed elements. The loader substitutes a random
  // name per document: a constant blooming across the DOM is visible to any
  // MutationObserver and identifies the product.
  const ATTR = '__OYA_ATTR__';

  // The most block rows an analysis hands over.
  const MAX_BLOCKS = 5000;

  // Text of a clickable card (a link wrapping title, price, rating) shown under
  // its tag when the tag's label cannot hold it.
  const MAX_WRAPPED_TEXT = 600;

  // Layout that starts a new line, and layout that sits beside its neighbours.
  // React and minified pages put no whitespace between elements, so without
  // these <div>$19.99</div><div>4.5 stars</div> reads as "$19.994.5 stars".
  const BLOCK_DISPLAY = /^(block|flex|grid|list-item|table|table-row|flow-root)$/;
  const SPACED_DISPLAY = /^(inline-block|inline-flex|inline-grid|table-cell)$/;

  // A dialog narrows the analysis to itself only when it blocks the page: marked
  // modal, opened with showModal(), a known site modal, or covering at least
  // this share of the viewport. A chat widget or popover leaves the page readable.
  const MODAL_SELECTOR = '[aria-modal="true"], .artdeco-modal__content, [data-testid="sheetDialog"]';
  const MODAL_MIN_SHARE = 0.5;

  // A panel that scrolls its own content (Gmail, Slack, a chat list) is reported
  // when it covers this share of the viewport and has this much more to scroll.
  const PANEL_MIN_SHARE = 0.25;
  const PANEL_MIN_OVERFLOW_PX = 50;

  // A fixed strip across the top no taller than this share of the viewport is a
  // sticky header, not an overlay: clicks scroll their target clear of it.
  const HEADER_MAX_SHARE = 0.25;



  const COLORS = {
    link: '#22c55e', button: '#3b82f6', input: '#a855f7',
    select: '#f59e0b', textarea: '#06b6d4', editable: '#ec4899',
  };

  const SKIP_TAGS = new Set([
    'SCRIPT', 'STYLE', 'NOSCRIPT', 'SVG', 'PATH', 'LINK', 'META',
    'HEAD', 'OBJECT', 'EMBED', 'CANVAS', 'MAP', 'TEMPLATE',
    'PICTURE', // skip <picture>, the <img> inside will be caught
  ]);

  const LANDMARK_TAGS = { HEADER: 'header', FOOTER: 'footer', NAV: 'nav', MAIN: 'main', ASIDE: 'aside', FORM: 'form', SECTION: 'section', ARTICLE: 'article' };

  const INTERACTIVE_ROLES = new Set([
    'button', 'link', 'textbox', 'combobox', 'tab', 'menuitem',
    'menuitemcheckbox', 'menuitemradio', 'option', 'checkbox', 'radio',
    'switch', 'slider', 'spinbutton', 'searchbox', 'gridcell', 'treeitem',
  ]);

  const INTERACTIVE_CHILD_SELECTOR = [
    'a[href]', 'button', 'input:not([type="hidden"])', 'select', 'textarea',
    'summary', '[role="button"]', '[role="link"]', '[role="textbox"]',
    '[role="checkbox"]', '[role="radio"]', '[role="switch"]', '[role="tab"]',
    '[role="menuitem"]', '[role="combobox"]', '[role="option"]', '[role="treeitem"]',
    '[onclick]', '[ng-click]', '[data-action]', '[jsaction]',
    '[data-control-name]', '[data-click]',
    // Site-specific
    '[data-testid]',                  // X/Twitter
    '[data-click-id]',                // Reddit
    '[data-tracking-control-name]',   // LinkedIn
  ].join(', ');

  let elementCounter = 0;
  let elementMap = [];
  const elementRefs = new Map(); // id → DOM node (survives React re-renders)

  window.analyzePage = function (options = {}) {
    // Bypass ClientRects noise from fingerprint spoofing during analysis
    // Runs in an isolated world, so the page's patched getBoundingClientRect
    // does not apply here and measurements are already unnoised.
    return _analyzePageInner(options);
  };

  function _analyzePageInner(options = {}) {
    cleanup();
    elementCounter = 0;
    elementMap = [];
    elementRefs.clear();

    // Resolve root — auto-detect open modal dialogs
    let root;
    let activeModal = null;

    if (options.selector) {
      root = document.querySelector(options.selector);
    } else {
      // Find visible modal dialogs — iterate backwards to find the topmost one.
      // Also catches LinkedIn overlays, Amazon popups, Reddit lightboxes.
      const modals = document.querySelectorAll(
        '[role="dialog"][aria-modal="true"], [role="alertdialog"][aria-modal="true"], dialog[open], ' +
        '[role="dialog"]:not([aria-modal="false"]), ' +  // Some sites omit aria-modal
        '.artdeco-modal__content, ' +                     // LinkedIn modals
        '[data-testid="sheetDialog"]'                     // X/Twitter sheets
      );
      for (let i = modals.length - 1; i >= 0; i--) {
        const m = modals[i];
        const rect = m.getBoundingClientRect();
        // Must be visible: has size and is not display:none/visibility:hidden
        if (rect.width > 0 && rect.height > 0 && !isHardHidden(m) && isShownOnTop(m, rect) && isModal(m, rect)) {
          activeModal = m;
          break;
        }
      }
      // documentElement stands in while a new document is still being parsed,
      // when body does not exist yet: a page mid-navigation is still readable.
      root = activeModal || document.body || document.documentElement;
    }

    if (!root) return { ok: false, error: `Root not found: ${options.selector}` };

    const vw = window.innerWidth, vh = window.innerHeight;
    const scrollX = window.scrollX, scrollY = window.scrollY;
    const pageH = document.documentElement.scrollHeight;
    const scrollPct = pageH > vh ? Math.round((scrollY / (pageH - vh)) * 100) : 0;

    let focusedId = null;
    const focused = document.activeElement;
    if (focused && focused !== document.body) {
      const aid = focused.getAttribute(ATTR);
      if (aid) focusedId = parseInt(aid, 10);
    }

    blocks = [];
    buffer = '';
    const top = { region: '' };
    walk(root, top);
    flush(top);

    for (const el of elementMap) {
      const dom = queryShadow(el.selector);
      if (dom) {
        const rect = dom.getBoundingClientRect();
        const off = getIframeOffset(dom);
        const top = rect.top + off.y, bottom = rect.bottom + off.y;
        const left = rect.left + off.x, right = rect.right + off.x;
        el.visible = bottom > 0 && top < vh && right > 0 && left < vw && rect.width > 0 && rect.height > 0;
        if (el.visible && !off.x && !off.y && isCovered(dom, rect)) {
          el.covered = true;
          el.state = el.state ? el.state + ' covered' : 'covered';
        }
      } else {
        el.visible = false;
      }
    }

    if (!focusedId && focused && focused !== document.body) {
      const aid = focused.getAttribute(ATTR);
      if (aid) focusedId = parseInt(aid, 10);
    }

    const visibleCount = elementMap.filter(e => e.visible).length;
    const coveredCount = elementMap.filter(e => e.covered).length;
    // Element rows' state, now that where each element is (on screen, covered) is known.
    for (const row of blocks) if (row.entry) { row.state = stateWords(row.entry); delete row.entry; }
    const panel = scrollPanel(vw, vh);
    const facts = {
      url: location.href,
      title: document.title,
      viewport: `${vw}x${vh}`,
      scroll: `${scrollPct}% (${scrollY}px of ${pageH}px)`,
      panelScroll: panel ? `${Math.round((panel.scrollTop / (panel.scrollHeight - panel.clientHeight)) * 100)}% (${Math.round(panel.scrollTop)}px of ${panel.scrollHeight}px); the content scrolls inside a panel` : '',
      elements: `${elementMap.length} total, ${visibleCount} visible`,
      modal: activeModal ? `${activeModal.getAttribute('aria-label') || activeModal.getAttribute('aria-labelledby') || 'unnamed'} (only this dialog was read)` : '',
      covered: coveredCount ? `${coveredCount} visible elements are behind something drawn over them (close it first)` : '',
      focused: focusedId || '',
    };
    const truncated = blocks.length > MAX_BLOCKS;
    if (truncated) facts.truncated = `showing ${MAX_BLOCKS} of ${blocks.length} blocks; scroll and analyze again for the rest`;

    if (options.highlight !== false) addHighlights();

    return {
      ok: true,
      data: {
        url: location.href, title: document.title,
        viewport: { width: vw, height: vh },
        scroll: { x: scrollX, y: scrollY, percent: scrollPct, pageHeight: pageH },
        focusedElement: focusedId,
        modal: activeModal ? (activeModal.getAttribute('aria-label') || true) : null,
        truncated,
        facts,
        blocks: blocks.slice(0, MAX_BLOCKS),
        elements: elementMap,
      },
    };
  };

  /**
   * Whether something else is drawn over the element's centre, such as a cookie
   * banner or an overlay. A click there lands on that instead.
   */
  function isCovered(dom, rect) {
    const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
    if (!hit || dom.contains(hit)) return false;
    for (let n = dom; n; n = n.parentNode || n.host) if (n === hit) return false;
    return !isStickyHeader(hit);
  }

  /** Whether an element sits in a fixed or sticky strip across the top of the viewport. */
  function isStickyHeader(el) {
    for (let n = el; n && n.nodeType === Node.ELEMENT_NODE; n = n.parentElement) {
      const position = window.getComputedStyle(n).position;
      if (position !== 'fixed' && position !== 'sticky') continue;
      const r = n.getBoundingClientRect();
      return r.top <= 0 && r.bottom <= window.innerHeight * HEADER_MAX_SHARE;
    }
    return false;
  }

  /**
   * Whether a dialog is really in front of the person: nothing above it is
   * transparent or hidden from assistive tech, it is on screen, and it is what is
   * drawn at its own centre. Sites keep closed login and cookie dialogs in the page
   * (aria-modal and all); reading only one of those hid the whole page.
   */
  function isShownOnTop(m, rect) {
    for (let n = m; n && n.nodeType === Node.ELEMENT_NODE; n = n.parentElement) {
      if (n.getAttribute('aria-hidden') === 'true' || getComputedStyle(n).opacity === '0') return false;
    }
    const x = Math.min(Math.max(rect.left + rect.width / 2, 0), window.innerWidth - 1);
    const y = Math.min(Math.max(rect.top + rect.height / 2, 0), window.innerHeight - 1);
    if (rect.bottom <= 0 || rect.right <= 0 || rect.top >= window.innerHeight || rect.left >= window.innerWidth) return false;
    const hit = document.elementFromPoint(x, y);
    return !!hit && (m.contains(hit) || hit.contains(m));
  }

  /** Whether an open dialog blocks the rest of the page, so reading only it is right. */
  function isModal(m, rect) {
    if (m.matches(MODAL_SELECTOR)) return true;
    try { if (m.matches(':modal')) return true; } catch {}
    return rect.width * rect.height >= window.innerWidth * window.innerHeight * MODAL_MIN_SHARE;
  }

  /**
   * The largest element that scrolls its own content. On apps that scroll
   * inside a panel the document never scrolls, so without this the header says
   * 0% and the agent thinks it has seen everything.
   */
  function scrollPanel(vw, vh) {
    let best = null, bestArea = vw * vh * PANEL_MIN_SHARE;
    for (const el of document.querySelectorAll('*')) {
      if (el === document.documentElement || el === document.body) continue;
      if (el.scrollHeight - el.clientHeight < PANEL_MIN_OVERFLOW_PX) continue;
      const area = el.clientWidth * el.clientHeight;
      if (area < bestArea) continue;
      const overflow = window.getComputedStyle(el).overflowY;
      if (overflow === 'auto' || overflow === 'scroll' || overflow === 'overlay') { best = el; bestArea = area; }
    }
    return best;
  }

  // ─── Site Detection ───

  function isHackerNews() {
    return location.hostname === 'news.ycombinator.com';
  }

  // ─── Page → blocks ───
  // The page as data: one row per heading, paragraph, list item, table row, image
  // and interactive element, in reading order, each with its region, kind, text,
  // target and state. Renderers outside the page (scripts/page-render.cjs) write
  // the rows as markdown, TOON or any other format; nothing here is format.

  const HEADINGS = { H1: 'h1', H2: 'h2', H3: 'h3', H4: 'h4', H5: 'h5', H6: 'h6' };
  const TEXT_KINDS = { LI: 'item', BLOCKQUOTE: 'quote', FIGCAPTION: 'caption', CAPTION: 'caption', DT: 'term' };

  // Query parameters that only track the click: noise in a link's target.
  const TRACKING_PARAM = /^(utm_\w+|trk\w*|ref|ref_src|fbclid|gclid|mc_cid|mc_eid|_ga|igshid|si)$/i;

  let blocks = [];   // rows so far, in reading order
  let buffer = '';   // inline text not yet in a row

  /** Ends the text gathered so far as one row of the context's kind (dropped where the context is muted). */
  function flush(ctx) {
    const text = buffer.replace(/\s+/g, ' ').trim();
    buffer = '';
    // Punctuation left between inline links ("," ", and") says nothing on its own.
    if (text && !ctx.mute && /[\p{L}\p{N}]/u.test(text)) blocks.push({ region: ctx.region, kind: ctx.kind || 'text', text });
  }

  /** Whether an element is left out: not content, hidden, the analyzer's own overlay, or aria-hidden and unseen. */
  function skipped(node) {
    if (SKIP_TAGS.has(node.tagName) || isHardHidden(node)) return true;
    if (node.id === 'ac-labels' || node.id === 'ac-highlight-style') return true;
    // Only aria-hidden elements that are also unseen: LinkedIn marks main content aria-hidden while messaging is open.
    if (node.getAttribute('aria-hidden') !== 'true') return false;
    const r = node.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return true;
    try { return window.getComputedStyle(node).opacity === '0'; } catch { return false; }
  }

  /** A node's children, its open shadow root's instead when it has one. */
  const childrenOf = (node) => (node.shadowRoot || node).childNodes;
  /** Form fields whose own element is the target, whatever they contain. */
  const isLeafField = (tag) => tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA';

  /** The context inside `node`: a landmark (nav, main, a named form or section) starts a region. */
  function regionOf(node, ctx) {
    const landmark = LANDMARK_TAGS[node.tagName] || landmarkFromRole(node);
    const label = (node.getAttribute('aria-label') || node.getAttribute('name') || '').trim().slice(0, 40);
    // An unnamed section says nothing; named, it tells the agent which part of the page this is.
    if (!landmark || (landmark === 'section' && !label)) return ctx;
    return { ...ctx, region: label ? `${landmark}/${label}` : landmark };
  }

  /** Walks one node, adding its rows. */
  function walk(node, ctx) {
    if (node.nodeType === Node.TEXT_NODE) { buffer += node.textContent; return; }
    if (node.nodeType !== Node.ELEMENT_NODE || skipped(node)) return;
    const tag = node.tagName;
    const type = getInteractiveType(node);
    // A wrapper around real controls is walked; the controls are the rows.
    if (type && (isLeafField(tag) || !hasInteractiveChild(node))) return addElement(node, type, ctx);
    const inner = regionOf(node, ctx);
    if (Object.hasOwn(SPECIAL, tag) && SPECIAL[tag](node, inner, ctx)) return;
    walkBlock(node, inner, ctx);
  }

  /** Walks an ordinary element: a block boundary or heading ends the text around it; inline spacing is kept. */
  function walkBlock(node, inner, ctx) {
    const kind = HEADINGS[node.tagName] || TEXT_KINDS[node.tagName];
    let display = '';
    try { display = window.getComputedStyle(node).display; } catch {}
    const block = !!kind || inner !== ctx || BLOCK_DISPLAY.test(display);
    const spaced = SPACED_DISPLAY.test(display);
    if (block) flush(ctx);
    if (spaced) buffer += ' ';
    const within = kind ? { ...inner, kind } : inner;
    for (const c of childrenOf(node)) walk(c, within);
    if (block) flush(within);
    else if (spaced) buffer += ' ';
  }

  /** Elements rendered their own way; each returns true once handled. */
  const SPECIAL = {
    IFRAME: (node, ctx) => walkFrame(node, ctx),
    PRE: (node, ctx) => {
      flush(ctx);
      // innerText keeps the lines a <br> or a block inside the <pre> makes; textContent runs them together.
      const text = (node.innerText || node.textContent).trim();
      if (text) blocks.push({ region: ctx.region, kind: 'code', text });
      return true;
    },
    IMG: (node, ctx) => {
      // An image with no alt says nothing, and pages are full of them.
      const alt = (node.getAttribute('alt') || '').trim();
      if (alt) { flush(ctx); blocks.push({ region: ctx.region, kind: 'image', text: alt }); }
      return true;
    },
    TABLE: (node, ctx) => !isHackerNews() && !isLayoutTable(node) && !isPickerGrid(node) && tableBlocks(node, ctx),
    DETAILS: (node, ctx) => detailsBlocks(node, ctx),
    SUMMARY: () => true,
    LABEL: (node, ctx) => labelBlocks(node, ctx),
    BR: () => { buffer += ' '; return true; },
    HR: (node, ctx) => { flush(ctx); return true; },
    TIME: (node) => {
      buffer += ' ' + (node.getAttribute('datetime') || node.getAttribute('title') || node.textContent.trim()) + ' ';
      return true;
    },
    SLOT: (node, ctx) => {
      for (const c of node.assignedNodes ? node.assignedNodes({ flatten: true }) : []) walk(c, ctx);
      return true;
    },
  };

  /** A same-origin iframe is read as part of the page; another site's is one row naming it. */
  function walkFrame(node, ctx) {
    flush(ctx);
    let doc = null;
    try { doc = node.contentDocument; } catch {}
    if (doc?.body) {
      const inner = { ...ctx, region: 'iframe' };
      for (const c of doc.body.childNodes) walk(c, inner);
      flush(inner);
    } else if (node.src) {
      blocks.push({ region: ctx.region, kind: 'iframe', target: node.src.slice(0, 120) });
    }
    return true;
  }

  /** A data table: a row per table row, cells separated by " | "; hidden rows and cells (a small-screen column) left out. */
  function tableBlocks(el, ctx) {
    flush(ctx);
    for (const tr of el.querySelectorAll(':scope > tr, :scope > thead > tr, :scope > tbody > tr, :scope > tfoot > tr')) {
      if (isHardHidden(tr)) continue;
      const cells = [...tr.querySelectorAll(':scope > th, :scope > td')].filter((c) => !isHardHidden(c));
      const row = { ...ctx, kind: cells.every((c) => c.tagName === 'TH') ? 'header' : 'row' };
      cells.forEach((cell, i) => {
        if (i) buffer += ' | ';
        // A | inside a cell is escaped, so it is never read as a cell boundary.
        const start = buffer.length;
        for (const c of childrenOf(cell)) walk(c, row);
        buffer = buffer.slice(0, start) + buffer.slice(start).replace(/\|/g, '\\|');
      });
      flush(row);
    }
    return true;
  }

  /** A <details>: its summary as a button, collapsed or expanded, and its body only when open. */
  function detailsBlocks(node, ctx) {
    flush(ctx);
    const summary = node.querySelector(':scope > summary');
    if (summary) addElement(summary, 'button', ctx, node.hasAttribute('open') ? 'expanded' : 'collapsed');
    if (!node.hasAttribute('open')) return true;
    for (const c of node.childNodes) if (c !== summary) walk(c, ctx);
    flush(ctx);
    return true;
  }

  /**
   * A label's text, unless its field's row already carries it: a label wrapping its
   * checkbox leaves only the checkbox's row, and a label pointing at a field
   * elsewhere is dropped, because the field is named by it.
   */
  function labelBlocks(node, ctx) {
    const control = node.control;
    const named = control && (control.hasAttribute('aria-label') || control.hasAttribute('aria-labelledby'));
    if (!control || named) return false;
    if (!node.contains(control)) return !isHardHidden(control);
    flush(ctx);
    const muted = { ...ctx, mute: true };
    for (const c of childrenOf(node)) walk(c, muted);
    flush(muted);
    return true;
  }

  /** An interactive element's row, then a clickable card's text that its name could not hold. */
  function addElement(node, type, ctx, state) {
    flush(ctx);
    const entry = registerElement(node, type);
    if (state) entry.state = [entry.state, state].filter(Boolean).join(' ');
    blocks.push({ id: entry.id, region: ctx.region, kind: elementKind(node, type), text: elementText(node, entry), target: elementTarget(node, type), entry });
    if (type !== 'editable' && !isLeafField(node.tagName)) cardText(node, entry, ctx);
  }

  /** The row kind: an input by its input type (input:email), anything else by what it is. */
  const elementKind = (node, type) => (type === 'input' ? `input:${(node.type || 'text').toLowerCase()}` : type);

  /** The row's text: the element's name, with Hacker News context where its name alone is ambiguous. */
  function elementText(node, entry) {
    const label = entry.text || '';
    if (!isHackerNews()) return label;
    if (label === 'reply') {
      const user = node.closest('.athing')?.querySelector('.hnuser')?.textContent;
      if (user) return `reply to ${user}`;
    }
    if (/^\d+\s*comment/.test(label)) {
      const title = node.closest('tr')?.previousElementSibling?.querySelector('.titleline a')?.textContent?.slice(0, 40);
      if (title) return `${label} on "${title}"`;
    }
    return label;
  }

  /** Where a link goes, or what a field holds now (never a password), or a select's chosen option. */
  function elementTarget(node, type) {
    if (type === 'link') {
      try {
        const u = new URL(node.href || '', location.origin);
        for (const key of [...u.searchParams.keys()]) if (TRACKING_PARAM.test(key)) u.searchParams.delete(key);
        return (u.hostname === location.hostname ? u.pathname + u.search : u.hostname + u.pathname + u.search).slice(0, 80);
      } catch { return ''; }
    }
    if (type === 'select') return node.options?.[node.selectedIndex]?.text?.trim() || '';
    if (type === 'editable') {
      const val = node.innerText?.replace(/\s+/g, ' ').trim() || '';
      return val.length > 200 ? val.slice(0, 197) + '...' : val;
    }
    if (type !== 'input' && type !== 'textarea' || !node.value) return '';
    return node.type === 'password' ? '••••' : String(node.value).slice(0, 80);
  }

  /** A clickable card (a link around a product's title, price and rating): the text its name could not hold. */
  function cardText(node, entry, ctx) {
    const label = entry.text || '';
    const lines = (node.innerText || '').split('\n').map((l) => l.replace(/\s+/g, ' ').trim()).filter(Boolean);
    const full = lines.join(' ');
    if (full.length <= label.length || label.includes(full)) return;
    blocks.push({ region: ctx.region, kind: 'text', text: full.slice(0, MAX_WRAPPED_TEXT) + (full.length > MAX_WRAPPED_TEXT ? '…' : '') });
  }

  /** An element row's state words: what the page says about it now, its expected format, and where it is. */
  function stateWords(entry) {
    const words = [];
    if (entry.checked !== undefined && ['checkbox', 'radio'].includes(entry.type)) words.push(entry.checked ? 'checked' : 'unchecked');
    if (entry.choiceOf) words.push(entry.choiceOf);
    if (entry.disabled) words.push('disabled');
    if (entry.required) words.push('required');
    if (entry.readOnly) words.push('readonly');
    if (entry.invalid || entry.error) words.push(entry.error ? `invalid: ${entry.error}` : 'invalid');
    if (entry.state) words.push(entry.state);
    const hint = entry.options || (entry.placeholder && entry.placeholder !== entry.text ? entry.placeholder : '');
    if (hint) words.push(`hint: ${String(hint).slice(0, 120)}`);
    if (!entry.visible) words.push('off-screen');
    return words.join('; ');
  }

  // ─── Interactive Element Detection ───

  function hasClickHandler(node) {
    if (node.hasAttribute('onclick')) return true;
    if (node.hasAttribute('onmousedown')) return true;
    if (node.hasAttribute('ng-click')) return true;
    if (node.hasAttribute('data-action')) return true;
    if (node.hasAttribute('jsaction')) return true;
    if (node.hasAttribute('data-control-name')) return true;  // LinkedIn
    if (node.hasAttribute('data-click')) return true;
    // LinkedIn: ember-style actions, feed controls
    if (node.hasAttribute('data-urn')) return true;
    if (node.hasAttribute('data-tracking-control-name')) return true;
    // X / Twitter: React event delegation
    if (node.hasAttribute('data-testid')) {
      const tid = node.getAttribute('data-testid');
      if (/like|retweet|reply|bookmark|share|follow|tweet/i.test(tid)) return true;
    }
    // Reddit: custom interactive elements
    if (node.tagName === 'SHREDDIT-POST' || node.tagName === 'FACEPLATE-TRACKER') return true;
    if (node.hasAttribute('data-click-id')) return true;  // Reddit
    if (node.hasAttribute('data-faceplate-tracking-context')) return true;  // Reddit
    // Amazon: interactive product elements
    if (node.hasAttribute('data-action')) return true;
    if (node.hasAttribute('data-cel-widget')) return true;
    if (node.hasAttribute('data-asin')) return true;
    return false;
  }

  function getInteractiveType(node) {
    const tag = node.tagName;
    const hidden = hiddenControl(node);
    if (hidden) return hidden.type.toLowerCase();

    // Native HTML interactive tags
    if (tag === 'A') return 'link';
    if (tag === 'BUTTON') return 'button';
    if (tag === 'SELECT') return 'select';
    if (tag === 'TEXTAREA') return 'textarea';

    if (tag === 'INPUT') {
      const t = (node.type || 'text').toLowerCase();
      if (t === 'hidden') return null;
      if (t === 'submit' || t === 'button' || t === 'reset' || t === 'image') return 'button';
      if (t === 'checkbox') return 'checkbox';
      if (t === 'radio') return 'radio';
      return 'input';
    }

    // Contenteditable — check before ARIA roles so rich-text editors
    // (e.g. LinkedIn post editor: div[role="textbox"][contenteditable])
    // are typed as 'editable' instead of 'input'.
    // Only match the element with the attribute, NOT inherited children.
    if (node.getAttribute('contenteditable') === 'true') {
      return 'editable';
    }

    // ARIA roles
    const role = node.getAttribute('role');
    if (role && INTERACTIVE_ROLES.has(role)) {
      if (role === 'link') return 'link';
      if (role === 'textbox' || role === 'searchbox') return 'input';
      if (role === 'combobox' || role === 'listbox') return 'select';
      if (role === 'checkbox' || role === 'switch') return 'checkbox';
      if (role === 'radio') return 'radio';
      return 'button';
    }

    // Explicit click handler attributes
    if (hasClickHandler(node)) return 'button';

    // A date picker's day: no role, no handler, no pointer cursor of its own.
    if (isPickerDay(node)) return 'button';

    // Tabindex: only interactive if also has cursor:pointer
    const tabindex = node.getAttribute('tabindex');
    if (tabindex !== null && tabindex !== '-1') {
      try { if (window.getComputedStyle(node).cursor === 'pointer') return 'button'; } catch {}
    }

    // cursor:pointer fallback — tighter constraints
    try {
      if (window.getComputedStyle(node).cursor === 'pointer' && node.textContent?.trim()) {
        const r = node.getBoundingClientRect();
        if (r.width > 0 && r.height > 0 && r.width < 400 && r.height < 120) return 'button';
      }
    } catch {}

    // ── Site-specific interactive element detection ──

    // LinkedIn: feed cards, connection actions, messaging items
    const cls = node.className || '';
    if (typeof cls === 'string') {
      // LinkedIn feed items and actions
      if (cls.includes('feed-shared-social-action') || cls.includes('artdeco-button') ||
          cls.includes('social-actions-button') || cls.includes('msg-conversation-card')) return 'button';
      // Reddit: vote buttons, expand/collapse
      if (cls.includes('voteButton') || cls.includes('_1rZYMD_4xY3gRcSS3p8ODO')) return 'button';
      // Amazon: add-to-cart, buy-now area elements
      if (cls.includes('a-button') || cls.includes('s-product-image-container')) return 'button';
    }

    // HackerNews: vote links (they use <a> without href but with onclick via id)
    if (tag === 'A' && !node.href && node.id?.startsWith('up_')) return 'button';

    return null;
  }

  function hasInteractiveChild(node) {
    for (const c of node.querySelectorAll(INTERACTIVE_CHILD_SELECTOR))
      if (!isHardHidden(c)) return true;
    return false;
  }

  // ─── Annotate interactive element ───

  /**
   * The handles that survive a reload: what a playbook step stores about an element
   * and what the server matches on later (server/src/modules/playbooks/service.ts matchElement).
   * `text` is passed in where the caller already has it — getLabel walks the DOM.
   */
  function stableOf(node, type, text = getLabel(node, type)) {
    const el = { type, tag: node.tagName.toLowerCase(), text };
    const role = node.getAttribute('role') || (node.tagName === 'BUTTON' ? 'button' : node.tagName === 'A' ? 'link' : undefined);
    if (role) el.role = role;
    if (node.href) el.href = node.href;
    // The attribute as written: a CSS locator matches it, not the resolved URL.
    const rawHref = node.getAttribute('href');
    if (rawHref) el.rawHref = rawHref;
    if (node.placeholder) el.placeholder = node.placeholder;
    if (node.id) el.domId = node.id;
    if (node.name) el.name = node.name;
    // Which box in a group: grids give each row's checkbox a fresh id per render,
    // but its value (the row's record id) stays.
    if ((node.type === 'checkbox' || node.type === 'radio') && node.getAttribute('value')) el.choice = node.getAttribute('value');
    const ariaLabel = node.getAttribute('aria-label');
    if (ariaLabel) el.ariaLabel = ariaLabel;
    const testId = node.getAttribute('data-testid');
    if (testId) el.testId = testId;
    // Always, as the last resort: a name the page repeats (a column header in a
    // sticky copy, "Cancel" in every dialog) leaves replay nothing unique without it.
    el.path = cssPath(node);
    const stable = stableName(node);
    if (stable && stable !== text) el.stableText = stable;
    const { selector, repeats } = scopedText(node, text);
    if (selector) el.scoped = selector;
    // A name the page repeats ("View Order" on every row) is ambiguous on replay: say so.
    if (repeats) el.repeats = 'true';
    return el;
  }

  /**
   * The name without what changes between visits: screen-reader-only text ("1 new
   * notification") and number badges ("1", "12+"). LinkedIn's nav link reads
   * "1 1 new notification Notifications" today and "Notifications" tomorrow; a
   * replay can only find it again by "Notifications".
   */
  function stableName(node) {
    const parts = [];
    const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT);
    for (let t = walker.nextNode(); t; t = walker.nextNode()) {
      const value = t.textContent.replace(/\s+/g, ' ').trim();
      if (!value || /^\d+\+?$/.test(value) || screenReaderOnly(t.parentElement, node)) continue;
      parts.push(value);
    }
    return parts.join(' ').slice(0, 80);
  }

  /** Whether an element (below `root`) is hidden from sight but read to screen readers. */
  function screenReaderOnly(el, root) {
    for (let n = el; n && n !== root.parentElement; n = n.parentElement) {
      const s = getComputedStyle(n);
      if (s.display === 'none' || s.visibility === 'hidden') return true;
      const clipped = (s.clip && s.clip !== 'auto') || (s.clipPath && s.clipPath !== 'none');
      const r = n.getBoundingClientRect();
      if (s.position === 'absolute' && (clipped || (r.width <= 1 && r.height <= 1))) return true;
    }
    return false;
  }

  // Attributes a page keeps stable across loads, in the order they identify a container best.
  const ANCHOR_ATTRS = ['id', 'data-testid', 'data-index', 'data-role', 'name', 'aria-label'];
  // How far up the recorder looks for a container that makes a repeated text unique.
  const SCOPE_WALK = 12;

  /** A selector for a container, from its first stable attribute, when that picks out only it. */
  function anchorOf(node) {
    for (const attr of ANCHOR_ATTRS) {
      const value = node.getAttribute(attr);
      if (!value) continue;
      const selector = `[${attr}=${JSON.stringify(value)}]`;
      if (document.querySelectorAll(selector).length === 1) return selector;
    }
    return null;
  }

  /**
   * When the element's text repeats on the page ("Content", "Cancel", a column
   * name in a sticky header copy): the text inside the nearest stable container
   * where it is unique, as a Playwright selector. Positions shift as a page
   * builds itself; a container named by its own attribute does not.
   */
  function scopedText(node, text) {
    if (!text || text.length > 80 || !node.isConnected || node.getRootNode() !== document) return {};
    const tag = node.localName;
    // Counted the way a replay's text locator counts: any element whose own text is
    // exactly this (the innermost one), not just elements of the clicked tag. The
    // admin menu's "Content" makes a section title "Content" ambiguous.
    const norm = (n) => (n.textContent || '').replace(/\s+/g, ' ').trim();
    const own = (n) => norm(n) === text && ![...n.children].some((c) => norm(c) === text);
    const matches = (root) => [...root.getElementsByTagName('*')].filter(own).length;
    if (matches(document) <= 1) return {};
    for (let a = node.parentElement, i = 0; a && a !== document.body && i < SCOPE_WALK; a = a.parentElement, i++) {
      const anchor = anchorOf(a);
      if (anchor && matches(a) === 1) return { repeats: true, selector: `${anchor} ${tag}:text-is(${JSON.stringify(text)})` };
    }
    return { repeats: true };
  }

  /**
   * A CSS path to the element by position, from its nearest ancestor whose id is
   * unique on the page (a sticky header's clone repeats ids, so those are passed over).
   */
  function cssPath(node) {
    const parts = [];
    const root = node.getRootNode();
    const uniqueId = (n) => n.id && root.querySelectorAll?.(`[id="${CSS.escape(n.id)}"]`).length === 1;
    for (let n = node; n && n.nodeType === Node.ELEMENT_NODE && n !== document.body; n = n.parentElement) {
      if (uniqueId(n)) return [`[id="${CSS.escape(n.id)}"]`, ...parts].join(' > ');
      const same = [...(n.parentElement?.children || [])].filter(c => c.tagName === n.tagName);
      parts.unshift(same.length > 1 ? `${n.localName}:nth-of-type(${same.indexOf(n) + 1})` : n.localName);
    }
    return ['body', ...parts].join(' > ');
  }

  /** Tags an interactive element with its id and records what the agent and the recorder know about it. */
  function registerElement(node, type) {
    elementCounter++;
    const id = elementCounter;
    node.setAttribute(ATTR, String(id));
    elementRefs.set(id, node); // keep live reference for click/type
    const text = getLabel(node, type);
    const entry = { id, ...stableOf(node, type, text), selector: `[${ATTR}="${id}"]` };
    if (node.value !== undefined && node.value !== '') entry.value = node.type === 'password' ? '••••' : node.value;
    if (node.disabled) entry.disabled = true;
    if (['input', 'textarea', 'select'].includes(type)) Object.assign(entry, fieldEntry(node));
    const checked = isChecked(node);
    if (checked !== undefined) entry.checked = checked;
    const place = choiceOf(node);
    if (place) entry.choiceOf = place;
    if (node.getAttribute('aria-disabled') === 'true') entry.disabled = true;
    const state = ariaState(node);
    if (state) entry.state = state;
    const form = node.closest('form');
    if (form) entry.formName = form.getAttribute('aria-label') || form.getAttribute('name') || form.getAttribute('action') || '';
    elementMap.push(entry);
    return entry;
  }

  /**
   * The hidden checkbox or radio a visible label stands for. Star ratings,
   * Material and Chakra controls hide the input and show the label, so the
   * label is what a person clicks and what carries the control's state.
   */
  function hiddenControl(node) {
    const control = node.tagName === 'LABEL' ? node.control : null;
    if (!control || !isHardHidden(control)) return null;
    const type = (control.type || '').toLowerCase();
    return type === 'checkbox' || type === 'radio' ? control : null;
  }

  /**
   * Where a radio sits in its group, said in words, for a group whose choices
   * look alike: five identical stars, or a scale of dots, where the label text
   * alone cannot say which choice is which.
   */
  function choiceOf(node) {
    const control = hiddenControl(node) || node;
    if ((control.type || '').toLowerCase() !== 'radio' || !control.name) return '';
    const group = [...document.getElementsByName(control.name)];
    const at = group.indexOf(control);
    return group.length > 1 && at >= 0 ? `choice ${at + 1} of ${group.length}` : '';
  }

  /** A native checkbox's checked, a label's hidden control's, or a custom one's aria-checked. */
  function isChecked(node) {
    const control = hiddenControl(node) || node;
    if (control.checked !== undefined) return control.checked;
    const aria = node.getAttribute('aria-checked');
    return aria === null ? undefined : aria === 'true' || aria === 'mixed';
  }

  /**
   * State a custom widget keeps only in ARIA attributes: whether a menu is open,
   * which tab or option is selected, which nav link is the current page, whether
   * a toggle button is on. Without it the agent re-clicks what is already done.
   */
  function ariaState(node) {
    const attr = (name) => node.getAttribute(name);
    const parts = [];
    if (attr('aria-expanded') !== null) parts.push(attr('aria-expanded') === 'true' ? 'expanded' : 'collapsed');
    if (attr('aria-selected') === 'true') parts.push('selected');
    if (attr('aria-current') && attr('aria-current') !== 'false') parts.push('current');
    if (attr('aria-pressed') === 'true' || attr('aria-pressed') === 'mixed') parts.push('pressed');
    return parts.join(' ');
  }

  // ─── What a form field says about itself ───
  // The agent fills forms from these lines alone, so a field carries its label,
  // whether it is required, the format it expects (a mask such as __/__/____),
  // and the error the page shows for it. Without them it guesses and retries.

  /** The field's label, when it is more than its placeholder. */
  function fieldName(node, text) {
    return text && text !== node.placeholder && text !== node.name ? ` "${text}"` : '';
  }

  /** The error the page shows for a field: aria-describedby text, or a visible error in the field's group. */
  function fieldError(node) {
    const doc = node.ownerDocument || document;
    const ids = (node.getAttribute('aria-describedby') || '').split(/\s+/).filter(Boolean);
    const described = ids.map(i => doc.getElementById(i)).filter(e => e && !isHardHidden(e)).map(e => e.textContent).join(' ');
    const group = node.closest('.form-group, [class*="form-field"], [class*="FormField"], fieldset');
    const shown = group && [...group.querySelectorAll('[role="alert"], .help-block, .invalid-feedback, .error-message, [class*="error"]')].find(e => e !== node && !e.contains(node) && !isHardHidden(e) && e.textContent.trim());
    return (described.trim() || shown?.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 120);
  }

  /** What the element index needs about a field: its input type, whether it is required, read-only or invalid, the page's error, and a select's options. */
  function fieldEntry(node) {
    const facts = {};
    if (node.tagName === 'INPUT') facts.inputType = (node.type || 'text').toLowerCase();
    if (node.required || node.getAttribute('aria-required') === 'true') facts.required = true;
    if (node.readOnly) facts.readOnly = true;
    const error = fieldError(node);
    if (error) facts.error = error;
    if (error || node.getAttribute('aria-invalid') === 'true' || (node.value && node.validity && !node.validity.valid)) facts.invalid = true;
    const options = optionPreview(node);
    if (options) facts.options = options.slice(2);
    return facts;
  }

  /** Required, disabled, read-only and invalid, with the page's error when it shows one. */
  function fieldState(node) {
    const facts = fieldEntry(node);
    const parts = [];
    if (facts.required) parts.push('required');
    if (node.disabled) parts.push('disabled');
    if (facts.readOnly) parts.push('readonly');
    if (facts.invalid) parts.push(facts.error ? `invalid: "${facts.error}"` : 'invalid');
    return parts.length ? ' ' + parts.join(' ') : '';
  }

  /** An input or textarea: its label, value (never a password's), expected format and state. */
  function fieldFacts(node, text) {
    const parts = [fieldName(node, text)];
    if (node.value) parts.push(` value="${node.type === 'password' ? '••••' : node.value.slice(0, 80)}"`);
    if (node.placeholder && node.placeholder !== text) parts.push(` placeholder="${node.placeholder.slice(0, 40)}"`);
    if (node.maxLength > 0 && node.maxLength <= 100) parts.push(` maxlength=${node.maxLength}`);
    return parts.join('') + fieldState(node);
  }

  /** The first few options of a select, so the agent can pick one without opening it. */
  function optionPreview(node) {
    const texts = [...(node.options || [])].map(o => o.text.trim()).filter(Boolean);
    if (!texts.length) return '';
    const shown = texts.slice(0, 8).map(t => t.slice(0, 30)).join(' | ');
    return `: ${shown}${texts.length > 8 ? ' | …' : ''}`;
  }

  function getLabel(node, type) {
    const aria = node.getAttribute('aria-label');
    if (aria) return aria.trim().slice(0, 80);
    // aria-labelledby
    const labelledBy = node.getAttribute('aria-labelledby');
    if (labelledBy) {
      const parts = labelledBy.split(/\s+/).map(id => (node.ownerDocument || document).getElementById(id)?.textContent?.replace(/\s+/g, ' ').trim()).filter(Boolean);
      if (parts.length) return parts.join(' ').slice(0, 80);
    }
    if (node.id && ['input', 'checkbox', 'radio', 'select', 'textarea'].includes(type)) {
      // The field's own document: a label inside an iframe is not in the top one.
      const l = (node.ownerDocument || document).querySelector(`label[for="${CSS.escape(node.id)}"]`);
      const labelText = l?.textContent?.replace(/\s+/g, ' ').trim();
      if (labelText) return labelText.slice(0, 80);
    }
    // Parent <label> wrapping this element
    const parentLabel = node.closest('label');
    if (parentLabel) {
      const labelText = parentLabel.textContent.replace(node.textContent || '', '').trim();
      if (labelText) return labelText.slice(0, 80);
    }
    // X/Twitter: data-testid often has a semantic name
    const testId = node.getAttribute('data-testid');
    if (testId && !node.textContent?.trim()) return testId.replace(/[-_]/g, ' ').slice(0, 80);
    // LinkedIn: data-control-name
    const controlName = node.getAttribute('data-control-name');
    if (controlName && !node.textContent?.trim()) return controlName.replace(/[_-]/g, ' ').slice(0, 80);
    const direct = [];
    for (const c of node.childNodes) if (c.nodeType === Node.TEXT_NODE && c.textContent.trim()) direct.push(c.textContent.trim());
    // A name broken up by styling is still one name: a search list marks the part
    // that matched, so "<b>ANTHEM</b> - CA" must not read as "- CA".
    if (direct.length && inlineOnly(node) && shownText(node)) return shownText(node).slice(0, 80);
    // Only text that says something: the "-" between a price filter's two amounts is not its name.
    if (direct.length && /[\p{L}\p{N}]/u.test(direct.join(''))) return direct.join(' ').slice(0, 80);
    const t = shownText(node);
    if (t) return t.slice(0, 80);
    // Check title attribute (HN vote arrows use title="upvote")
    const title = node.getAttribute('title')?.trim();
    if (title) return title.slice(0, 80);
    // Check child element titles (HN: <a><div class="votearrow" title="upvote"></div></a>)
    const childTitle = node.querySelector('[title]');
    if (childTitle) return childTitle.getAttribute('title').trim().slice(0, 80);
    return iconName(node) || nearbyName(node) || node.placeholder?.slice(0, 80) || node.name || '';
  }

  /** Tags that only style the text they wrap, so a name split across them is still one name. */
  const INLINE_NAME_TAGS = new Set(
    ['MARK', 'STRONG', 'B', 'EM', 'I', 'SPAN', 'SMALL', 'U', 'SUP', 'SUB', 'CODE', 'ABBR', 'FONT', 'BDI', 'S', 'INS', 'DEL'],
  );

  /** Whether an element's children only style its text, so its whole text reads as its name. */
  const inlineOnly = (node) => [...node.children].every((c) => INLINE_NAME_TAGS.has(c.tagName));

  /** A class name's icon word: fa-search, icon-trash, glyphicon-plus. */
  function iconWord(node) {
    for (const kid of node.querySelectorAll('[class]')) {
      const cls = String(kid.className?.baseVal ?? kid.className ?? '');
      const found = /(?:^|\s)(?:fa|fas|far|fal|fab|icon|glyphicon|material-icons)[-\s]([a-z0-9-]{2,30})/i.exec(cls);
      if (found) return found[1];
    }
    return '';
  }

  /** What an icon says a control does, for a button that shows a picture and no words. */
  function iconName(node) {
    const titled = node.querySelector('svg title, [data-icon]');
    const title = titled?.textContent?.trim() || titled?.getAttribute?.('data-icon') || '';
    if (title) return title.replace(/[-_]/g, ' ').slice(0, 80);
    const use = node.querySelector('use');
    const sprite = (use?.getAttribute('href') || use?.getAttribute('xlink:href') || '').split('#')[1] || '';
    return (sprite || iconWord(node)).replace(/[-_]/g, ' ').slice(0, 80);
  }

  /**
   * What the row or card around a nameless control holds. A results list gives
   * its buttons ids like "selectProvider0", which say nothing about the record
   * they choose; the row's own text does.
   */
  function nearbyName(node) {
    const box = node.closest('tr, li, [role="row"], [class*="card"], [class*="result"]');
    if (!box || box === node) return '';
    const text = (box.innerText || box.textContent || '').replace(/\s+/g, ' ').trim();
    return /[\p{L}\p{N}]/u.test(text) ? text.slice(0, 80) : '';
  }

  /**
   * An element's visible text as its accessible name spells it. innerText skips
   * hidden text but applies CSS text-transform ("REPORTS"); when that is the only
   * difference, the text as written ("Reports") is what a replay's name match needs.
   */
  function shownText(node) {
    const shown = node.innerText?.replace(/\s+/g, ' ').trim() || '';
    const written = node.textContent?.replace(/\s+/g, ' ').trim() || '';
    return written && shown.toLowerCase() === written.toLowerCase() ? written : shown;
  }

  /**
   * A date picker's month grid. Its day cells carry no role, no handler and no
   * pointer cursor — the widget listens on the container — so the grid is left
   * to the ordinary walk, where each day registers as an element to click.
   */
  function isPickerGrid(table) {
    return !!table.closest('[class*="datepicker"], [class*="date-picker"], [class*="calendar"], [role="dialog"][class*="picker"]')
      && !!table.querySelector('td[class*="day"], td[class*="date"]');
  }

  /**
   * One day in a date picker's grid: clickable although nothing in the markup
   * says so. A day cell holds its number and nothing else, which keeps the
   * widget's own containers (datepicker-days and the like) out of it.
   */
  function isPickerDay(node) {
    if (!/^\d{1,2}$/.test((node.textContent || '').trim())) return false;
    const cls = typeof node.className === 'string' ? node.className : '';
    if (!/(^|\s)(day|date)(\s|$)|-day(\s|$)|-date(\s|$)/.test(cls)) return false;
    return !!node.closest('[class*="datepicker"], [class*="date-picker"], [class*="calendar"], [role="dialog"][class*="picker"]');
  }

  /** Detect layout tables (no <th>, used for positioning not data). */
  function isLayoutTable(table) {
    if (table.querySelector('th')) return false;
    if (table.getAttribute('role') === 'presentation' || table.getAttribute('role') === 'none') return true;
    // If most rows have just 1-2 cells with mixed content, it's probably layout
    const rows = table.querySelectorAll(':scope > tbody > tr, :scope > tr');
    if (rows.length === 0) return false;
    let layoutScore = 0;
    for (const row of rows) {
      const cells = row.querySelectorAll(':scope > td');
      if (cells.length <= 2) layoutScore++;
      // Cells containing interactive elements = layout
      for (const cell of cells) {
        if (cell.querySelector('a, button, input, img')) layoutScore++;
      }
    }
    return layoutScore > rows.length;
  }

  function landmarkFromRole(node) {
    const r = node.getAttribute('role');
    return r ? ({ banner: 'header', navigation: 'nav', main: 'main', complementary: 'aside', contentinfo: 'footer', form: 'form', region: 'section', search: 'search' })[r] || null : null;
  }

  function isHardHidden(node) {
    try {
      const s = window.getComputedStyle(node);
      if (s.display === 'none' || s.visibility === 'hidden') return true;
      // Catch elements moved off-screen (common on LinkedIn, Amazon for screen readers)
      if (s.position === 'absolute' || s.position === 'fixed') {
        const r = node.getBoundingClientRect();
        if (r.right < -100 || r.bottom < -100 || r.left > window.innerWidth + 100) return true;
      }
      return false;
    } catch { return false; }
  }

  /** Return {x, y} offset if element lives inside a same-origin iframe. */
  function getIframeOffset(el) {
    const ownerDoc = el.ownerDocument;
    if (ownerDoc === document) return { x: 0, y: 0 };
    for (const iframe of document.querySelectorAll('iframe')) {
      try {
        if (iframe.contentDocument === ownerDoc) {
          const r = iframe.getBoundingClientRect();
          return { x: r.left, y: r.top };
        }
      } catch {}
    }
    return { x: 0, y: 0 };
  }

  function queryShadow(selector, root = document) {
    const el = root.querySelector(selector);
    if (el) return el;
    for (const h of root.querySelectorAll('*')) {
      if (h.shadowRoot) { const f = queryShadow(selector, h.shadowRoot); if (f) return f; }
      if (h.tagName === 'IFRAME') {
        try {
          const iframeDoc = h.contentDocument;
          if (iframeDoc) { const f = queryShadow(selector, iframeDoc); if (f) return f; }
        } catch {}
      }
    }
    return null;
  }

  window.__acQueryShadow = queryShadow;

  // Element lookup: ID-only. Never falls back to CSS selectors.
  window.__acFindElement = function (selector) {
    // Accepts a number, or any selector carrying one — the attribute name is
    // per-document now, so the id identifies the element, not the name.
    const match = typeof selector === 'number'
      ? [null, String(selector)]
      : String(selector).match(/(\d+)/);
    if (!match) return null;

    const id = parseInt(match[1], 10);

    // 1. Live reference from elementRefs map
    const ref = elementRefs.get(id);
    if (ref && ref.isConnected) return ref;

    // 2. DOM query by data-ac-id (may have been re-attached by observer)
    const byAttr = document.querySelector(`[${ATTR}="${id}"]`);
    if (byAttr) { elementRefs.set(id, byAttr); return byAttr; }

    // 3. Shadow DOM + iframe search by data-ac-id
    const byShadow = queryShadow(`[${ATTR}="${id}"]`);
    if (byShadow) { elementRefs.set(id, byShadow); return byShadow; }

    // 4. Recovery: find replacement element by stored metadata
    const replacement = findReplacementElement(id);
    if (replacement) {
      replacement.setAttribute(ATTR, String(id));
      elementRefs.set(id, replacement);
      return replacement;
    }

    // 5. Last resort: try by DOM id from metadata
    const entry = elementMap.find(e => e.id === id);
    if (entry?.domId) {
      const byDomId = document.getElementById(entry.domId);
      if (byDomId) {
        byDomId.setAttribute(ATTR, String(id));
        elementRefs.set(id, byDomId);
        return byDomId;
      }
    }

    return null;
  };

  // ─── Highlight Overlays ───

  const HIGHLIGHT_CSS = `[${ATTR}]{outline:2px solid var(--ac-hl-color,#3b82f6)!important;outline-offset:1px!important}`;
  const LABEL_CSS = `.ac-label{position:absolute;font-family:ui-monospace,monospace;font-size:11px;font-weight:700;line-height:16px;padding:0 5px;border-radius:4px;color:#fff;z-index:2147483646;pointer-events:none;white-space:nowrap;box-shadow:0 1px 4px rgba(0,0,0,.5)}`;

  function addHighlights() {
    let style = document.getElementById('ac-highlight-style');
    if (!style) { style = document.createElement('style'); style.id = 'ac-highlight-style'; style.textContent = HIGHLIGHT_CSS + LABEL_CSS; document.head.appendChild(style); }
    // Inject highlight styles into same-origin iframes
    for (const iframe of document.querySelectorAll('iframe')) {
      try {
        const iframeDoc = iframe.contentDocument;
        if (iframeDoc && iframeDoc.head && !iframeDoc.getElementById('ac-highlight-style')) {
          const s = iframeDoc.createElement('style');
          s.id = 'ac-highlight-style';
          s.textContent = HIGHLIGHT_CSS;
          iframeDoc.head.appendChild(s);
        }
      } catch {}
    }
    let c = document.getElementById('ac-labels');
    if (c) c.remove();
    c = document.createElement('div');
    c.id = 'ac-labels';
    c.style.cssText = 'position:absolute;top:0;left:0;width:0;height:0;overflow:visible;z-index:2147483646;pointer-events:none';
    document.body.appendChild(c);
    const sx = window.scrollX, sy = window.scrollY;
    for (const el of elementMap) {
      const dom = queryShadow(el.selector);
      if (!dom) continue;
      const color = COLORS[el.type] || COLORS.button;
      dom.style.setProperty('--ac-hl-color', color);
      const rect = dom.getBoundingClientRect();
      if (!rect.width && !rect.height) continue;
      const off = getIframeOffset(dom);
      const lbl = document.createElement('div');
      lbl.className = 'ac-label';
      lbl.style.background = color;
      lbl.style.left = `${rect.left + off.x + sx - 2}px`;
      lbl.style.top = `${rect.top + off.y + sy - 18}px`;
      lbl.textContent = `${el.id} ${el.type}`;
      c.appendChild(lbl);
    }
  }

  // ─── Cleanup ───

  function cleanup() {
    const c = document.getElementById('ac-labels'); if (c) c.remove();
    const s = document.getElementById('ac-highlight-style'); if (s) s.remove();
    document.querySelectorAll(`[${ATTR}]`).forEach(el => { el.removeAttribute(ATTR); el.style.removeProperty('--ac-hl-color'); });
    // Clean up inside same-origin iframes
    document.querySelectorAll('iframe').forEach(iframe => {
      try {
        const iframeDoc = iframe.contentDocument;
        if (!iframeDoc) return;
        const iframeStyle = iframeDoc.getElementById('ac-highlight-style');
        if (iframeStyle) iframeStyle.remove();
        iframeDoc.querySelectorAll(`[${ATTR}]`).forEach(el => { el.removeAttribute(ATTR); el.style.removeProperty('--ac-hl-color'); });
      } catch {}
    });
  }

  window.__acCleanup = cleanup;

  // ─── Live DOM Observer ───
  // Auto-tags new interactive elements as they're added (React re-renders,
  // infinite scroll, dropdowns, modals, etc.) without needing a full re-analyze.

  let observerTimer = null;
  const pendingNodes = new Set();

  function processNewNodes() {
    observerTimer = null;
    if (!elementMap.length) return; // no analysis has run yet

    const labelContainer = document.getElementById('ac-labels');
    const sx = window.scrollX, sy = window.scrollY;
    const vw = window.innerWidth, vh = window.innerHeight;

    for (const node of pendingNodes) {
      if (node.nodeType !== Node.ELEMENT_NODE) continue;
      if (!node.isConnected) continue;
      if (node.id === 'ac-labels' || node.id === 'ac-highlight-style') continue;

      // Walk the new subtree for interactive elements
      const walker = document.createTreeWalker(node, NodeFilter.SHOW_ELEMENT);
      let el = node;
      while (el) {
        if (!el.hasAttribute(ATTR) && !SKIP_TAGS.has(el.tagName) && !isHardHidden(el)) {
          const type = getInteractiveType(el);
          if (type) {
            const isLeaf = el.tagName === 'INPUT' || el.tagName === 'SELECT' || el.tagName === 'TEXTAREA';
            if (isLeaf || !hasInteractiveChild(el)) {
              elementCounter++;
              const id = elementCounter;
              el.setAttribute(ATTR, String(id));
              elementRefs.set(id, el);
              const text = getLabel(el, type);
              const entry = { id, type, tag: el.tagName.toLowerCase(), selector: `[${ATTR}="${id}"]`, text };
              if (el.href) entry.href = el.href;
              if (el.disabled) entry.disabled = true;
              if (el.id) entry.domId = el.id;
              const rect = el.getBoundingClientRect();
              const off = getIframeOffset(el);
              const top = rect.top + off.y, bottom = rect.bottom + off.y;
              const left = rect.left + off.x, right = rect.right + off.x;
              entry.visible = bottom > 0 && top < vh && right > 0 && left < vw && rect.width > 0 && rect.height > 0;
              elementMap.push(entry);

              // Add highlight label
              if (labelContainer) {
                const color = COLORS[type] || COLORS.button;
                el.style.setProperty('--ac-hl-color', color);
                if (rect.width > 0 && rect.height > 0) {
                  const lbl = document.createElement('div');
                  lbl.className = 'ac-label';
                  lbl.style.background = color;
                  lbl.style.left = `${left + sx - 2}px`;
                  lbl.style.top = `${top + sy - 18}px`;
                  lbl.textContent = `${id} ${type}`;
                  labelContainer.appendChild(lbl);
                }
              }
            }
          }
        }
        el = walker.nextNode();
      }
    }
    pendingNodes.clear();
  }

  /** Find a replacement element in the DOM matching stored metadata. */
  function findReplacementElement(id) {
    const entry = elementMap.find(e => e.id === id);
    if (!entry) return null;

    // 1. By DOM id
    if (entry.domId) {
      const byId = document.getElementById(entry.domId);
      if (byId && !byId.hasAttribute(ATTR)) return byId;
    }
    // 2. By name attribute
    if (entry.name) {
      const byName = document.querySelector(`${entry.tag}[name="${CSS.escape(entry.name)}"]`);
      if (byName && !byName.hasAttribute(ATTR)) return byName;
    }
    // 3. By data-testid
    if (entry.testId) {
      const byTestId = document.querySelector(`[data-testid="${CSS.escape(entry.testId)}"]`);
      if (byTestId && !byTestId.hasAttribute(ATTR)) return byTestId;
    }
    // 4. By aria-label + tag
    if (entry.ariaLabel) {
      const byAria = document.querySelector(`${entry.tag}[aria-label="${CSS.escape(entry.ariaLabel)}"]`);
      if (byAria && !byAria.hasAttribute(ATTR)) return byAria;
    }
    // 5. By tag + text content match
    if (entry.text) {
      const candidates = document.querySelectorAll(entry.tag);
      for (const c of candidates) {
        if (c.hasAttribute(ATTR)) continue;
        const cText = getLabel(c, entry.type);
        if (cText === entry.text) return c;
      }
    }
    return null;
  }

  /** Re-attach data-ac-id to a re-rendered element. */
  function reattachElement(id) {
    const ref = elementRefs.get(id);
    if (ref && ref.isConnected) return; // still alive

    // Check if data-ac-id already exists in DOM (was re-rendered with it)
    const existing = document.querySelector(`[${ATTR}="${id}"]`);
    if (existing) { elementRefs.set(id, existing); return; }

    // Find replacement
    const replacement = findReplacementElement(id);
    if (replacement) {
      replacement.setAttribute(ATTR, String(id));
      elementRefs.set(id, replacement);
    }
  }

  const observer = new MutationObserver((mutations) => {
    for (const m of mutations) {
      if (m.type === 'childList') {
        // Track removed elements for SPA re-render recovery
        for (const node of m.removedNodes) {
          if (node.nodeType !== Node.ELEMENT_NODE) continue;
          const removed = node.querySelectorAll ? [node, ...node.querySelectorAll(`[${ATTR}]`)] : [node];
          for (const oldEl of removed) {
            const aid = oldEl.getAttribute?.(ATTR);
            if (!aid) continue;
            const id = parseInt(aid, 10);
            const ref = elementRefs.get(id);
            if (ref && !ref.isConnected) {
              // Element was removed — try to find replacement after addedNodes are processed
              setTimeout(() => reattachElement(id), 100);
            }
          }
        }
        for (const node of m.addedNodes) {
          if (node.nodeType === Node.ELEMENT_NODE) pendingNodes.add(node);
        }
      }
    }
    if (pendingNodes.size > 0 && !observerTimer) {
      observerTimer = setTimeout(processNewNodes, 200);
    }
  });

  // A document with no body (a frameset, an XML page) has nothing to watch; throwing
  // here would also stop the recorder below from ever listening.
  if (document.body) observer.observe(document.body, { childList: true, subtree: true });
  // ─── Recorder: what the person does, as playbook steps ───
  //
  // Same step shape the agent produces (server/src/modules/agent/chat.ts recordStep), so a
  // recording replays and exports to Playwright through the code that already exists.
  // The loader substitutes the flag, so a recording started on the previous page keeps
  // running in the document that replaces it.
  // ponytail: same-origin frames only. A cross-site iframe is a separate target the
  // recording channel does not attach to, so a flow inside one records nothing.
  const RECORD_ON = '__OYA_RECORD__' === 'true';

  const RECORD_KEYS = new Set(['Enter', 'Tab', 'Escape']);
  // Keys that move through listboxes, menus, tabs and date pickers. Recorded only
  // inside such a widget: in text they move the caret, on the page they scroll.
  const NAV_KEYS = new Set([' ', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown']);
  // How far up from the event target the recorder looks for what was clicked.
  const RECORD_WALK = 10;
  // A press whose element is gone before its click (menus and options that act on
  // pointerdown) is recorded after this wait, if it moved less than this far.
  const PRESS_CLICK_MS = 300;
  const PRESS_SLOP_PX = 10;

  const TEXTUAL = new Set(['input', 'textarea', 'editable']);
  const SECRET_AUTOCOMPLETE = /current-password|new-password|one-time-code/i;
  const MAX_RECORDED = 500;

  let recording = false;
  let recorded = [];
  let sequence = 0;
  let lastKey = null;
  const documentId = Math.random().toString(36).slice(2);
  const recordedSecrets = new Set();
  let typing = null;   // { node, el, value } — the field being typed into
  let focused = null;  // { node, el } — captured before typing, so a label is never the typed text

  // Check at event time, not when flushing: a real edit may hide/remove its field.
  // Do not require viewport intersection: keyboard users can focus scrolled content.
  // Transparent elements count for clicks and choices: styled checkboxes, switches
  // and selects are real controls at opacity 0 that the person clicked. Typing into
  // a field nobody can see is still never an edit.
  function recordVisible(node, transparentOk = false) {
    if (!node || node.nodeType !== Node.ELEMENT_NODE || !node.isConnected) return false;
    if (node.tagName === 'INPUT' && node.type === 'hidden') return false;
    for (let el = node; el; el = el.parentElement || el.getRootNode()?.host) {
      const style = getComputedStyle(el);
      if (style.display === 'none' || style.visibility === 'hidden'
        || style.visibility === 'collapse' || (style.opacity === '0' && !transparentOk)
        || style.contentVisibility === 'hidden') return false;
    }
    return [...node.getClientRects()].some(r => r.width > 0 && r.height > 0);
  }

  /** The element above `node`, crossing out of a shadow root to its host. */
  const parentOf = (node) => node.parentElement || node.getRootNode()?.host || null;

  // Elements that are interactive by what they are, not by how they look.
  const SEMANTIC = 'a, button, input, select, textarea, summary, [contenteditable="true"], [role]';

  /**
   * The nearest element above the target that is interactive by what it is: the
   * link around a menu item's <span>, the button around its label. A pointer
   * cursor makes the span look clickable too, but only the link has a name and a
   * role that a replay finds once.
   */
  function semanticTarget(start) {
    for (let node = start, i = 0; node && node.nodeType === Node.ELEMENT_NODE && i < RECORD_WALK; i++, node = parentOf(node)) {
      if (!node.matches(SEMANTIC)) continue;
      if (node.hasAttribute('role') && !INTERACTIVE_ROLES.has(node.getAttribute('role'))) continue;
      const type = getInteractiveType(node);
      if (type) return { node: coveredToggleLabel(node, type) || node, type };
    }
    return null;
  }

  /**
   * A styled switch or checkbox: the real input sits under its own label (a Yes/No
   * slider), so what the person clicked, and what a replay can click, is the label.
   */
  function coveredToggleLabel(node, type) {
    if (type !== 'checkbox' && type !== 'radio' || node.tagName !== 'INPUT') return null;
    const label = node.labels?.[0];
    if (!label || !recordVisible(label, true)) return null;
    const r = node.getBoundingClientRect();
    if (!r.width || !r.height) return label;
    const hit = node.getRootNode().elementFromPoint?.(r.x + r.width / 2, r.y + r.height / 2);
    return hit && hit !== node ? label : null;
  }

  /** The interactive element an event really landed on. */
  function recordTarget(event) {
    if (!event.isTrusted) return null;
    const start = (event.composedPath && event.composedPath()[0]) || event.target;
    const choice = ['click', 'pointerdown', 'change'].includes(event.type);
    if (!recordVisible(start, choice)) return null;
    const semantic = semanticTarget(start);
    if (semantic && !(event.type === 'click' && hiddenControlLabel(start, semantic.node))) return semantic;
    for (let node = start, i = 0; node && node.nodeType === Node.ELEMENT_NODE && i < RECORD_WALK; i++, node = parentOf(node)) {
      // Styled checkboxes often hide their native input. Replay the visible label;
      // the browser-forwarded click on its hidden control is excluded above.
      if (event.type === 'click' && node.tagName === 'LABEL' && node.control
        && !recordVisible(node.control)) return { node, type: 'button' };
      const type = getInteractiveType(node);
      if (type) return { node, type };
    }
    return pointerTarget(start);
  }

  /**
   * An icon button or a div with a script click handler: nothing marks it as
   * interactive but its pointer cursor. The outermost element of the pointer run
   * above the target is what the person meant.
   */
  function pointerTarget(start) {
    let found = null;
    for (let node = start, i = 0; node && node.nodeType === Node.ELEMENT_NODE && i < RECORD_WALK; i++, node = parentOf(node)) {
      if (getComputedStyle(node).cursor === 'pointer') found = node;
      else if (found) break;
    }
    return found ? { node: found, type: 'button' } : null;
  }

  /** The label of a hidden checkbox between the target and `until`: the visible thing the person clicked. */
  function hiddenControlLabel(start, until) {
    for (let node = start; node && node !== until; node = parentOf(node)) {
      if (node.tagName === 'LABEL' && node.control && !recordVisible(node.control)) return node;
    }
    return null;
  }

  /** A file input's change: the input is almost always hidden behind a button, but the choice is real. */
  function fileTarget(event) {
    const node = event.target;
    return event.isTrusted && node?.tagName === 'INPUT' && node.type === 'file' ? { node, type: 'input' } : null;
  }

  const isSecretField = (node) => String(node.type || '').toLowerCase() === 'password'
    || SECRET_AUTOCOMPLETE.test(node.getAttribute('autocomplete') || '')
    || /password|passwd|secret|token|otp|verification.?code|security.?code/i.test([node.name, node.id, node.getAttribute('aria-label')].join(' '));

  /** A password never leaves the page: the step keeps a placeholder, the name is flagged. */
  function secretPlaceholder(node) {
    const raw = node.getAttribute('name') || node.id || '';
    const name = /^[A-Za-z_]\w{0,39}$/.test(raw) && !['__proto__', 'constructor', 'prototype'].includes(raw) ? raw : 'password';
    recordedSecrets.add(name);
    return '{{' + name + '}}';
  }

  function pushStep(step) {
    if (!recording || recorded.length >= MAX_RECORDED) return;
    const entry = { ...step, t: step.t || Date.now(), id: documentId + ':' + (++sequence) };
    recorded.push(entry);
    if (window.__acRecordSink) {
      try {
        window.__acRecordSink({ steps: [entry], secrets: [...recordedSecrets] });
        recorded.pop();
      } catch { /* retain for the next drain */ }
    }
  }

  /** One `type` step per field, not one per keystroke. */
  function flushTyping() {
    if (!typing) return;
    const { node, el, value, t } = typing;
    typing = null;
    // Stamped at the last keystroke: a flush after control passed to the agent still counts.
    pushStep({ action: 'type', el, t, text: value !== '' && isSecretField(node) ? secretPlaceholder(node) : value });
  }

  function onRecordFocus(e) {
    if (!recording) return;
    const hit = recordTarget(e);
    if (!hit || !TEXTUAL.has(hit.type)) return;
    if (typing && typing.node !== hit.node) flushTyping();
    focused = { node: hit.node, el: stableOf(hit.node, hit.type) };
  }

  function onRecordInput(e) {
    if (!recording) return;
    const hit = recordTarget(e);
    if (!hit || !TEXTUAL.has(hit.type)) return;
    if (typing && typing.node !== hit.node) flushTyping();
    const raw = hit.type === 'editable' ? (hit.node.innerText || '') : (hit.node.value || '');
    const el = (typing && typing.node === hit.node && typing.el)
      || (focused && focused.node === hit.node && focused.el)
      || stableOf(hit.node, hit.type);
    typing = { node: hit.node, el, value: String(raw).slice(0, 2000), t: Date.now() };
  }

  let pressed = null;      // { hit, el, x, y, clicked } — the element under the last pointerdown

  /**
   * Menus, selects and options often act on pointerdown and remove themselves
   * before the click, which then lands on a container or nowhere. The press is
   * kept so the click can be recorded against what was really pressed.
   */
  function onRecordPointerDown(e) {
    if (!recording || e.button !== 0) return;
    const hit = recordTarget(e);
    pressed = hit ? { hit, el: stableOf(hit.node, hit.type), x: e.clientX, y: e.clientY, clicked: false } : null;
  }

  /** A press that no click follows (its element was removed first) is recorded as the click. */
  function onRecordPointerUp(e) {
    const press = pressed;
    if (!recording || !press || Math.hypot(e.clientX - press.x, e.clientY - press.y) > PRESS_SLOP_PX) return;
    setTimeout(() => {
      if (press.clicked || pressed !== press) return;
      pressed = null;
      recordClick(press.hit, press.el, 1);
    }, PRESS_CLICK_MS);
  }

  /** The element a click meant: the pressed one when the click lost it (removed, or retargeted to a container). */
  function clickHit(e) {
    const hit = recordTarget(e);
    const press = pressed;
    pressed = null;
    if (!press) return hit && { hit, el: null };
    press.clicked = true;
    const lost = !press.hit.node.isConnected || !hit || (hit.node !== press.hit.node && hit.node.contains(press.hit.node));
    return lost ? { hit: press.hit, el: press.el } : { hit, el: null };
  }

  /**
   * A click on the label of a checkbox or radio the recorder can see: the browser
   * forwards it to the control, and that forwarded click is the step. Recording
   * both would toggle it twice on replay.
   */
  function labelOfVisibleToggle(e) {
    const label = (e.composedPath?.() || []).find(n => n.tagName === 'LABEL');
    const control = label?.control;
    return !!control && control !== e.target && ['checkbox', 'radio'].includes(control.type) && recordVisible(control, true);
  }

  function onRecordClick(e) {
    if (!recording || labelOfVisibleToggle(e)) return;
    const found = clickHit(e);
    if (found) recordClick(found.hit, found.el, e.detail);
  }

  /** Records a click on `hit` (with `el` when it was captured before the element changed). */
  function recordClick(hit, el, detail) {
    // A click inside the field being typed into is the caret moving, not a step.
    if (typing && typing.node === hit.node) return;
    flushTyping();
    if (hit.node.tagName === 'SELECT') return; // the change event carries the option
    // Enter on a focused button, and Enter submitting a form, arrive as a key *and* as
    // a click the browser synthesized (detail 0). The key was already delivered,
    // so omit the synthesized click instead of replaying the submit twice.
    if (detail === 0 && lastKey && ['Enter', ' '].includes(lastKey.key) && Date.now() - lastKey.t < 1000) return;
    lastKey = null;
    pushStep({ action: 'click', el: el || stableOf(hit.node, hit.type) });
  }

  function onRecordChange(e) {
    if (!recording) return;
    const hit = fileTarget(e) || recordTarget(e);
    if (!hit) return;
    if (hit.node.type === 'file') { flushTyping(); pushStep({ action: 'upload_file', el: stableOf(hit.node, hit.type), file: '{{upload_file}}' }); }
    else if (hit.type === 'select') {
      flushTyping();
      const opt = hit.node.selectedOptions && hit.node.selectedOptions[0];
      if (opt) pushStep({ action: 'select_option', el: stableOf(hit.node, hit.type), option: String(opt.label || opt.textContent || '').trim() });
    } else if (TEXTUAL.has(hit.type)) {
      flushTyping();
    }
  }

  // Widgets whose keyboard use is part of the task: a key there picks or moves something.
  const KEY_WIDGETS = '[role="listbox"], [role="option"], [role="menu"], [role="menubar"], [role="menuitem"], '
    + '[role="grid"], [role="gridcell"], [role="tree"], [role="treeitem"], [role="tablist"], [role="tab"], '
    + '[role="radiogroup"], [role="slider"], [role="spinbutton"], [role="combobox"], input[type="radio"], input[type="range"]';

  /** Whether a navigation key does something of its own here, rather than move the caret, scroll, or activate a click. */
  function navKeyCounts(e, target) {
    if (target.isContentEditable || target.tagName === 'SELECT') return false;
    const type = getInteractiveType(target);
    if (TEXTUAL.has(type) || !target.closest?.(KEY_WIDGETS)) return false;
    // Space on a button, link or checkbox fires a click, which is the step.
    return e.key !== ' ' || !['button', 'link', 'checkbox'].includes(type);
  }

  function onRecordKey(e) {
    if (!recording || !e.isTrusted) return;
    const target = e.composedPath?.()[0] || e.target;
    const nav = NAV_KEYS.has(e.key);
    if (!RECORD_KEYS.has(e.key) && !(nav && navKeyCounts(e, target))) return;
    if (!recordVisible(target)) return;
    flushTyping();  // the value is the step; the key is what submits it
    lastKey = { key: e.key, t: Date.now() };
    const name = e.key === ' ' ? 'Space' : e.key;
    const key = [e.ctrlKey && 'Control', e.metaKey && 'Meta', e.altKey && 'Alt', e.shiftKey && 'Shift', name].filter(Boolean).join('+');
    pushStep({ action: 'press_key', key });
  }

  /** Leaving the page (address bar, back, closing the tab) ends the typing in progress. */
  function onRecordLeave() {
    if (document.visibilityState === 'hidden') flushTyping();
  }

  function onUnsupportedInteraction(event) {
    if (!recording || !event.isTrusted || !recordVisible(event.composedPath?.()[0] || event.target)) return;
    flushTyping();
    pushStep({ action: 'unsupported_' + event.type, captureIssue: 'This ' + event.type + ' interaction needs a manual step before replay.' });
  }
  document.addEventListener('drop', onUnsupportedInteraction, true);
  document.addEventListener('click', event => { if (event.target?.tagName === 'CANVAS') onUnsupportedInteraction(event); }, true);
  document.addEventListener('focusin', onRecordFocus, true);
  document.addEventListener('input', onRecordInput, true);
  document.addEventListener('change', onRecordChange, true);
  document.addEventListener('click', onRecordClick, true);
  document.addEventListener('pointerdown', onRecordPointerDown, true);
  document.addEventListener('pointerup', onRecordPointerUp, true);
  document.addEventListener('keydown', onRecordKey, true);
  document.addEventListener('visibilitychange', onRecordLeave, true);
  window.addEventListener('pagehide', flushTyping, true);

  window.__acRecordStart = function () {
    if (!recording) { recorded = []; recordedSecrets.clear(); typing = null; focused = null; lastKey = null; pressed = null; }
    recording = true; return true;
  };
  window.__acRecordClear = function () {
    recorded = []; recordedSecrets.clear(); typing = null; focused = null; lastKey = null; pressed = null;
  };
  window.__acRecordStop = function () { flushTyping(); recording = false; return true; };

  /** Steps since the last call, and every secret name seen. Clears the step buffer. */
  window.__acRecordDrain = function (final) {
    if (final) flushTyping();
    const steps = recorded;
    recorded = [];
    return { steps, secrets: [...recordedSecrets] };
  };

  if (RECORD_ON) recording = true;
})();
