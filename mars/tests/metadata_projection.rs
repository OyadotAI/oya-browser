/** Full metadata survives the public Eye-to-V9 seam and the ordinary saved-state round trip. */
use mars::{document, v9};

/** Prepare a real parser projection without services, transport, native UI or network fixtures. */
fn execute(
    document: &document::Document,
    environment: &v9::PageEnvironment,
    source: &str,
) -> Result<v9::Execution, mars_core::protocol::EngineError> {
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_core::canvas::Unavailable,
        languages: &[],
        fragments: &document::DomFragments,
    };
    v9::load_page(v9::PageInput {
        sources: &[source.into()],
        source_keys: &[],
        module_keys: &[],
        document: document::script_document(document, "metadata-page").unwrap(),
        storage: Default::default(),
        host: &host,
        environment,
        modules: v9::ModuleInput::NONE,
    })
}

/** Both old caps are exceeded while scripts still read the complete body data block in place. */
#[test]
fn large_data_and_hundreds_of_metadata_elements_reach_scripts_and_restore() {
    let prefix = (0..300)
        .map(|index| format!("<meta id=m{index} content={index}>"))
        .collect::<String>();
    let data = "x".repeat(2 * 1024 * 1024);
    let parsed = document::parse(&format!("<head>{prefix}</head><body><section id=holder><script type=application/json id=data>\"{data}\"</script><span id=after></span></section></body>")).unwrap();
    assert_eq!(parsed.metadata.len(), 301);
    let environment = v9::PageEnvironment {
        metadata: parsed.metadata.clone(),
        ..Default::default()
    };
    let execution = execute(&parsed, &environment, "const data=document.getElementById('data');if(data.textContent.length!==2097154)throw Error('truncated data');if(document.getElementById('m299').getAttribute('content')!=='299')throw Error('missing metadata');if(data.parentNode.id!=='holder'||data.nextElementSibling.id!=='after')throw Error('wrong placement');console.log('complete');").unwrap();
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(execution.effects.console.last().unwrap().text, "complete");
    let encoded = serde_json::to_vec(&execution.state).unwrap();
    let restored: v9::VmState = serde_json::from_slice(&encoded).unwrap();
    restored
        .validate_document(&document::script_document(&parsed, "metadata-page").unwrap())
        .unwrap();
}

/** A forged environment is checked even when it did not originate in Eye's parser. */
#[test]
fn oversized_environment_metadata_is_refused_before_bootstrap() {
    let parsed = document::parse("<meta id=config>").unwrap();
    let mut environment = v9::PageEnvironment {
        metadata: parsed.metadata.clone(),
        ..Default::default()
    };
    environment.metadata[0].text = "x".repeat(mars_core::constants::METADATA_BYTES);
    assert!(execute(&parsed, &environment, "throw Error('must not execute')").is_err());
}
