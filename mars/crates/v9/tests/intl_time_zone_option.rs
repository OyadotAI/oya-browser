/** The timeZone option must name a zone: values that are not zone names throw a RangeError, while UTC, offsets and Area/Location names are accepted. */
mod common;

/** A number is not a zone name, so the constructor throws a RangeError naming it. */
#[test]
fn numeric_time_zone_throws_range_error() {
    assert_eq!(
        common::eval(
            "let r;try{new Intl.DateTimeFormat('en-US',{timeZone:0});r='none'}catch(e){r=e.name+': '+e.message}",
            "r"
        ),
        "RangeError: Invalid time zone specified: 0"
    );
}

/** A boolean is not a zone name either. */
#[test]
fn boolean_time_zone_throws_range_error() {
    assert_eq!(
        common::eval(
            "let r;try{new Intl.DateTimeFormat('en-US',{timeZone:true});r='none'}catch(e){r=e.name}",
            "r"
        ),
        "RangeError"
    );
}

/** An object is converted with ToString first, so an object whose toString gives UTC is accepted. */
#[test]
fn object_time_zone_is_converted_before_validation() {
    assert_eq!(
        common::eval(
            "const zone={toString(){return 'UTC'}};const r=new Intl.DateTimeFormat('en-US',{timeZone:zone}).resolvedOptions().timeZone;",
            "r"
        ),
        "UTC"
    );
}

/** An Area/Location identifier is accepted and reported as written. */
#[test]
fn area_location_time_zone_is_accepted() {
    assert_eq!(
        common::eval(
            "const r=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York'}).resolvedOptions().timeZone;",
            "r"
        ),
        "America/New_York"
    );
}

/** Mixed-case names resolve through ICU's preferred identifiers, including country-specific links. */
#[test]
fn names_and_aliases_resolve_to_preferred_identifiers() {
    let source = "const names=['eUrope/isLe_OF_man','africa/Dar_eS_salaam','America/port_of_spain','America/Argentina/Buenos_Aires','America/Buenos_Aires','Asia/Kathmandu','Asia/Katmandu','Asia/Ulan_Bator'];const r=names.map(timeZone=>new Intl.DateTimeFormat('en',{timeZone}).resolvedOptions().timeZone).join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "Europe/Isle_of_Man|Africa/Dar_es_Salaam|America/Port_of_Spain|America/Buenos_Aires|America/Buenos_Aires|Asia/Katmandu|Asia/Katmandu|Asia/Ulaanbaatar"
    );
}

/** Unknown names and malformed offsets fail rather than silently using the host zone. */
#[test]
fn invalid_identifiers_and_offsets_throw() {
    let source = "const bad=['Aurope/Paris','America//New_York','+24:00','+01:60','+01:00:00','+😀'];const r=bad.map(timeZone=>{try{new Intl.DateTimeFormat('en',{timeZone});return false}catch(e){return e instanceof RangeError}}).join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true,true");
}

/** The resolved zone and its output share offset rules across winter, summer and fixed offsets. */
#[test]
fn formatting_uses_the_zone_at_each_instant() {
    let source = "const options={hour:'2-digit',minute:'2-digit',hour12:false};const f=(timeZone,t)=>new Intl.DateTimeFormat('en',{...options,timeZone}).format(t);const winter=Date.UTC(2024,0,1,12),summer=Date.UTC(2024,6,1,12);const r=[f('Europe/Isle_of_Man',winter),f('Europe/Isle_of_Man',summer),f('Asia/Kathmandu',summer),f('+0530',summer),f('-03',summer),new Intl.DateTimeFormat('en',{timeZone:'-00'}).resolvedOptions().timeZone].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "12:00|13:00|17:45|17:30|09:00|+00:00"
    );
}

/** Both sides of a DST transition use the actual offset across all formatting entry points. */
#[test]
fn daylight_transition_is_consistent_across_formatting_methods() {
    let source = "const f=new Intl.DateTimeFormat('en',{timeZone:'Europe/London',hour:'2-digit',minute:'2-digit',hour12:false,timeZoneName:'short'});const before=Date.UTC(2024,2,31,0,59),after=Date.UTC(2024,2,31,1,0);const a=f.format(before),b=f.format(after);const r=[a,b,f.formatToParts(after).map(p=>p.value).join('')===b,f.formatRange(before,after)].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "00:59 UTC|02:00 GMT+01:00|true|00:59 UTC – 02:00 GMT+01:00"
    );
}

/** Object-to-string failures propagate and the default zone is consistently UTC. */
#[test]
fn coercion_errors_propagate_and_default_zone_is_utc() {
    let source = "let caught=false;try{new Intl.DateTimeFormat('en',{timeZone:{toString(){throw 'bad'}}})}catch(e){caught=e==='bad'}const f=new Intl.DateTimeFormat('en',{hour:'2-digit',minute:'2-digit',hour12:false});const r=[caught,f.resolvedOptions().timeZone,f.format(Date.UTC(2024,0,1,12))].join('|');";
    assert_eq!(common::eval(source, "r"), "true|UTC|12:00");
}

/** Historical offsets include seconds in both the formatted time and the offset label. */
#[test]
fn historical_offset_labels_preserve_seconds() {
    let source = "const t=Date.UTC(1900,0,1),o={hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,timeZoneName:'longOffset'};const r=['Europe/Paris','Africa/Monrovia'].map(timeZone=>new Intl.DateTimeFormat('en',{...o,timeZone}).format(t)).join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "00:09:21 GMT+00:09:21|23:16:52 GMT-00:43:08"
    );
}

/** Other Intl constructors do not read or validate the DateTimeFormat-only option. */
#[test]
fn unrelated_intl_constructors_never_read_time_zone() {
    let source = "let reads=0;const o={get timeZone(){reads++;throw 'bad'}};const a=[Intl.Collator,Intl.Segmenter,Intl.ListFormat,Intl.RelativeTimeFormat];const r=a.map(C=>{try{new C('en',o);return true}catch(e){return false}}).join()+'|'+reads;";
    assert_eq!(common::eval(source, "r"), "true,true,true,true|0");
}

/** Zone labels remain visible when only date fields or a long time style were requested. */
#[test]
fn zone_names_are_not_lost_without_explicit_time_fields() {
    let source = "const t=Date.UTC(2024,0,1);const a=new Intl.DateTimeFormat('en',{timeZone:'Asia/Kathmandu',year:'numeric',timeZoneName:'longOffset'}).format(t);const b=new Intl.DateTimeFormat('en',{timeZone:'Asia/Kathmandu',timeStyle:'long',hour12:false}).format(t);const r=[a,b].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "2024, GMT+05:45|05:45:00 GMT+05:45"
    );
}
