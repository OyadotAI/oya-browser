/** Canvas image APIs must copy actual pixels with WebIDL conversions and branded storage. */
use crate::common;
use mars_core::canvas::CanvasHost;
use mars_v9::{DocumentHost, PageInput, StorageState};

/** Pixel fixtures deliberately require no OS graphics or external resources. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Every fixture executes the public JavaScript API against a real Canvas-capable VM. */
fn run(source: &str) -> String {
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let execution = mars_v9::load_page(PageInput {
        sources: &[source.into()],
        source_keys: &[],
        module_keys: &[],
        document: common::page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .unwrap();
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}

#[test]
fn constructors_preserve_supplied_array_identity_and_zero_new_storage() {
    assert_eq!(
        run(
            "const bytes=new Uint8ClampedArray([1,2,3,4,5,6,7,8]);const d=new ImageData(bytes,2);const z=new ImageData(1,2);const r=[d instanceof ImageData,d.data===bytes,d.width,d.height,d.colorSpace,d.pixelFormat,Array.from(z.data)].join('|')"
        ),
        "true|true|2|1|srgb|rgba-unorm8|0,0,0,0,0,0,0,0"
    );
}

/** Readback rejects out-of-range coordinates before coercing later arguments or allocating pixels. */
#[test]
fn readback_enforces_signed_long_coordinates_without_wrapping() {
    assert_eq!(
        run(
            "const x=document.createElement('canvas').getContext('2d');let r='';for(const v of [NaN,Infinity,2147483648,-2147483649]){try{x.getImageData(v,0,1,1)}catch(e){r+=e.name+','}}let called=false;try{x.getImageData(Infinity,{valueOf(){called=true;return 0}},1,1)}catch(e){}r+='|'+called+'|'+x.getImageData(0.9,0,1.9,1).width"
        ),
        "TypeError,TypeError,TypeError,TypeError,|false|1"
    );
}

/** Finite inputs whose arithmetic overflows cannot abort execution outside JavaScript catch. */
#[test]
fn unrepresentable_drawing_geometry_is_catchable_and_preserves_pixels() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');c.width=1;c.height=1;const x=c.getContext('2d');x.fillStyle='red';x.fillRect(0,0,1,1);let r='';for(const draw of [()=>x.fillRect(1e308,0,1e308,1),()=>x.clearRect(1e308,0,1e308,1),()=>x.drawImage(c,1e308,0,1e308,1)]){try{draw()}catch(e){r+=e.name+','}}x.setTransform(2,0,0,1,0,0);try{x.fillRect(1e308,0,1,1)}catch(e){r+=e.name}r+='|after|'+Array.from(x.getImageData(0,0,1,1).data)"
        ),
        "NotSupportedError,NotSupportedError,NotSupportedError,NotSupportedError|after|255,0,0,255"
    );
}

#[test]
fn constructors_reject_wrong_array_shapes_and_unsupported_formats() {
    assert_eq!(
        run(
            "let out=[];for(const f of [()=>ImageData(1,1),()=>new ImageData(0,1),()=>new ImageData(new Uint8Array(4),1),()=>new ImageData(new Uint8ClampedArray(3),1),()=>new ImageData(new Uint8ClampedArray(8),3),()=>new ImageData(new Uint8ClampedArray(8),2,2),()=>new ImageData(1,1,{colorSpace:'display-p3'})]){try{f()}catch(e){out.push(e.name)}}const r=out.join('|')"
        ),
        "TypeError|IndexSizeError|TypeError|InvalidStateError|IndexSizeError|IndexSizeError|NotSupportedError"
    );
}

#[test]
fn image_data_subclasses_keep_constructor_prototypes_and_brands() {
    assert_eq!(
        run(
            "class Data extends ImageData{}const d=new Data(1,1);let forged='';try{Object.getOwnPropertyDescriptor(ImageData.prototype,'data').get.call({})}catch(e){forged=e.name}const r=[d instanceof Data,d instanceof ImageData,d.width,forged].join('|')"
        ),
        "true|true|1|TypeError"
    );
}

#[test]
fn create_image_data_copies_dimensions_without_aliasing_or_pixels() {
    assert_eq!(
        run(
            "const a=document.createElement('canvas').getContext('2d');const d=new ImageData(new Uint8ClampedArray([1,2,3,4]),1);const x=a.createImageData(d),y=a.createImageData(-2,-1);const r=[x.width,x.height,x.data!==d.data,Array.from(x.data),y.width,y.height].join('|')"
        ),
        "1|1|true|0,0,0,0|2|1"
    );
}

#[test]
fn pixel_insertion_ignores_transform_alpha_and_smoothing() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');c.width=2;c.height=1;const a=c.getContext('2d');a.translate(1,0);a.globalAlpha=0;a.imageSmoothingEnabled=false;const d=new ImageData(new Uint8ClampedArray([10,20,30,40,50,60,70,80]),2);a.putImageData(d,0,0);const r=Array.from(a.getImageData(0,0,2,1).data).join(',')"
        ),
        "10,20,30,40,50,60,70,80"
    );
}

#[test]
fn dirty_rectangles_normalize_negative_extents_and_clip_both_bitmaps() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');c.width=2;c.height=1;const a=c.getContext('2d');const d=new ImageData(new Uint8ClampedArray([10,0,0,255,20,0,0,255,30,0,0,255]),3);a.putImageData(d,-1,0,3,1,-3,-1);const r=Array.from(a.getImageData(0,0,2,1).data).join(',')"
        ),
        "20,0,0,255,30,0,0,255"
    );
}

#[test]
fn detached_image_data_is_rejected_before_destination_mutation() {
    assert_eq!(
        run(
            "const a=document.createElement('canvas').getContext('2d');a.fillStyle='red';a.fillRect(0,0,1,1);const d=new ImageData(1,1);d.data.buffer.transfer();let error='';try{a.putImageData(d,0,0)}catch(e){error=e.name}const r=error+'|'+Array.from(a.getImageData(0,0,1,1).data)"
        ),
        "InvalidStateError|255,0,0,255"
    );
}

#[test]
fn coercion_can_resize_destination_before_pixel_insertion() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas'),a=c.getContext('2d');const d=new ImageData(new Uint8ClampedArray([1,2,3,255]),1);a.putImageData(d,{valueOf(){c.width=1;c.height=1;return 0}},0);const r=Array.from(a.getImageData(0,0,1,1).data).join(',')"
        ),
        "1,2,3,255"
    );
}

#[test]
fn canvas_self_copy_uses_a_snapshot_before_overlapping_writes() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');c.width=3;c.height=1;const a=c.getContext('2d');a.imageSmoothingEnabled=false;a.putImageData(new ImageData(new Uint8ClampedArray([10,0,0,255,20,0,0,255,30,0,0,255]),3),0,0);a.drawImage(c,0,0,2,1,1,0,2,1);const r=Array.from(a.getImageData(0,0,3,1).data).join(',')"
        ),
        "10,0,0,255,10,0,0,255,20,0,0,255"
    );
}

#[test]
fn copied_pixels_survive_later_source_resize_and_mutation() {
    assert_eq!(
        run(
            "const s=document.createElement('canvas'),d=document.createElement('canvas');s.width=d.width=1;s.height=d.height=1;const a=s.getContext('2d'),b=d.getContext('2d');a.fillStyle='blue';a.fillRect(0,0,1,1);b.drawImage(s,0,0);s.width=1;a.fillStyle='red';a.fillRect(0,0,1,1);const r=Array.from(b.getImageData(0,0,1,1).data).join(',')"
        ),
        "0,0,255,255"
    );
}

#[test]
fn source_cropping_adjusts_destination_placement() {
    assert_eq!(
        run(
            "const s=document.createElement('canvas'),d=document.createElement('canvas');s.width=2;d.width=3;s.height=d.height=1;const a=s.getContext('2d'),b=d.getContext('2d');a.putImageData(new ImageData(new Uint8ClampedArray([255,0,0,255,0,0,255,255]),2),0,0);b.imageSmoothingEnabled=false;b.drawImage(s,-1,0,3,1,0,0,3,1);const r=Array.from(b.getImageData(0,0,3,1).data).join(',')"
        ),
        "0,0,0,0,255,0,0,255,0,0,255,255"
    );
}

#[test]
fn image_copy_uses_transform_and_global_alpha() {
    assert_eq!(
        run(
            "const s=document.createElement('canvas'),d=document.createElement('canvas');s.width=s.height=1;d.width=2;d.height=1;s.getContext('2d').fillRect(0,0,1,1);const b=d.getContext('2d');b.translate(1,0);b.globalAlpha=.5;b.drawImage(s,0,0,1,1);const r=Array.from(b.getImageData(0,0,2,1).data).join(',')"
        ),
        "0,0,0,0,0,0,0,128"
    );
}

#[test]
fn image_copy_reacquires_source_dimensions_after_conversion() {
    assert_eq!(
        run(
            "const s=document.createElement('canvas'),d=document.createElement('canvas');s.width=2;s.height=1;d.width=2;d.height=1;const a=s.getContext('2d'),b=d.getContext('2d');b.drawImage(s,{valueOf(){s.width=1;a.fillStyle='red';a.fillRect(0,0,1,1);return 0}},0);const r=Array.from(b.getImageData(0,0,2,1).data).join(',')"
        ),
        "255,0,0,255,0,0,0,0"
    );
}

#[test]
fn blank_sources_are_transparent_and_non_canvas_sources_fail_explicitly() {
    assert_eq!(
        run(
            "const s=document.createElement('canvas'),d=document.createElement('canvas'),b=d.getContext('2d');b.fillStyle='red';b.fillRect(0,0,1,1);b.drawImage(s,0,0);let error='';try{b.drawImage({},0,0)}catch(e){error=e.name}const r=error+'|'+Array.from(b.getImageData(0,0,1,1).data)"
        ),
        "NotSupportedError|255,0,0,255"
    );
}

#[test]
fn solid_color_forms_preserve_alpha_and_invalid_assignments() {
    assert_eq!(
        run(
            "const a=document.createElement('canvas').getContext('2d');let r='';for(const color of ['#f008','#ff000080','rgba(255,0,0,0.5)','rgb(100% 0% 0% / 50%)']){a.clearRect(0,0,1,1);a.fillStyle=color;a.fillRect(0,0,1,1);r+=a.getImageData(0,0,1,1).data[3]+','}a.fillStyle='rgba(1,2,3,NaN)';r+='|'+a.fillStyle"
        ),
        "136,128,128,128,|rgba(255, 0, 0, 0.502)"
    );
}

#[test]
fn image_coordinates_enforce_range_while_constructor_dimensions_wrap() {
    assert_eq!(
        run(
            "const a=document.createElement('canvas').getContext('2d'),d=new ImageData(4294967297,1);let out=[];for(const f of [()=>a.putImageData(d,4294967296,0),()=>a.createImageData(4294967297,1),()=>a.putImageData(d,NaN,0),()=>a.createImageData(Infinity,1)]){try{f()}catch(e){out.push(e.name)}}a.putImageData(d,.9,-.9);const r=d.width+'|'+out.join(',')"
        ),
        "1|TypeError,TypeError,TypeError,TypeError"
    );
}

#[test]
fn detached_and_shrunken_views_cannot_construct_or_supply_pixels() {
    assert_eq!(
        run(
            "const a=document.createElement('canvas').getContext('2d');const buffer=new ArrayBuffer(8,{maxByteLength:16});const bytes=new Uint8ClampedArray(buffer,0,8),d=new ImageData(bytes,2);buffer.resize(4);let out=[];for(const f of [()=>a.putImageData(d,0,0),()=>new ImageData(bytes,2)]){try{f()}catch(e){out.push(e.name)}}const other=new Uint8ClampedArray(4);other.buffer.transfer();try{new ImageData(other,1)}catch(e){out.push(e.name)}const r=out.join('|')"
        ),
        "InvalidStateError|InvalidStateError|InvalidStateError"
    );
}

#[test]
fn insertion_uses_private_image_dimensions_and_typed_view_offsets() {
    assert_eq!(
        run(
            "const a=document.createElement('canvas').getContext('2d');const bytes=new Uint8ClampedArray([9,9,9,9,1,2,3,255]);const d=new ImageData(new Uint8ClampedArray(bytes.buffer,4,4),1);Object.defineProperty(d,'width',{value:100});Object.defineProperty(d,'data',{value:new Uint8ClampedArray(0)});a.putImageData(d,0,0);const r=Array.from(a.getImageData(0,0,1,1).data).join(',')"
        ),
        "1,2,3,255"
    );
}

#[test]
fn image_scaling_observes_smoothing_and_negative_extents_do_not_flip() {
    assert_eq!(
        run(
            "const s=document.createElement('canvas'),d=document.createElement('canvas');s.width=2;s.height=d.height=1;d.width=4;const a=s.getContext('2d'),b=d.getContext('2d');a.putImageData(new ImageData(new Uint8ClampedArray([255,0,0,255,0,0,255,255]),2),0,0);b.imageSmoothingEnabled=false;b.drawImage(s,4,0,-4,1);const nearest=Array.from(b.getImageData(0,0,4,1).data);b.clearRect(0,0,4,1);b.imageSmoothingEnabled=true;b.drawImage(s,0,0,4,1);const smooth=b.getImageData(1,0,1,1).data;const r=nearest.join(',')+'|'+smooth[0]+','+smooth[2]+','+smooth[3]"
        ),
        "255,0,0,255,255,0,0,255,0,0,255,255,0,0,255,255|191,64,255"
    );
}

#[test]
fn numeric_coercion_detachment_is_checked_before_any_pixel_write() {
    assert_eq!(
        run(
            "const a=document.createElement('canvas').getContext('2d'),d=new ImageData(1,1);let error='';try{a.putImageData(d,{valueOf(){d.data.buffer.transfer();return 0}},0)}catch(e){error=e.name}const r=error+'|'+a.getImageData(0,0,1,1).data[3]"
        ),
        "InvalidStateError|0"
    );
}
