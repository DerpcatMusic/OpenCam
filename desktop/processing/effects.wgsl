struct Params { dimensions:vec4<u32>, effects:vec4<f32>, local:vec4<f32>, aspect:vec4<f32>, flags:vec4<u32> }
@group(0) @binding(0) var<storage,read> source:array<u32>;
@group(0) @binding(1) var<storage,read_write> blurA:array<u32>;
@group(0) @binding(2) var<storage,read_write> blurB:array<u32>;
@group(0) @binding(3) var<storage,read_write> output:array<u32>;
@group(0) @binding(4) var<storage,read> mask:array<f32>;
@group(0) @binding(5) var<uniform> p:Params;
fn read_pixel(pos:vec2<u32>, kind:u32) -> vec4<f32> {
    let i=pos.y*p.dimensions.x+pos.x;
    if kind==1u {return unpack4x8unorm(blurA[i]);}
    if kind==2u {return unpack4x8unorm(blurB[i]);}
    return unpack4x8unorm(source[i]);
}
fn sample_pixel(uv:vec2<f32>,kind:u32) -> vec4<f32> {
    let d=p.dimensions.xy;let f=clamp(uv*vec2<f32>(d)-0.5,vec2<f32>(0.),vec2<f32>(d)-1.);
    let q=vec2<u32>(floor(f));let next=min(q+vec2<u32>(1u),d-vec2<u32>(1u));let t=fract(f);
    return mix(mix(read_pixel(q,kind),read_pixel(vec2<u32>(next.x,q.y),kind),t.x),mix(read_pixel(vec2<u32>(q.x,next.y),kind),read_pixel(next,kind),t.x),t.y);
}
fn mask_value(uv:vec2<f32>) -> f32 {
    if p.flags.z==0u {return 0.;}
    let f=clamp(uv*vec2<f32>(256.,144.)-0.5,vec2<f32>(0.),vec2<f32>(255.,143.));let q=vec2<u32>(floor(f));let n=min(q+vec2<u32>(1u),vec2<u32>(255u,143u));let t=fract(f);
    return smoothstep(0.25,0.75,mix(mix(mask[q.y*256u+q.x],mask[q.y*256u+n.x],t.x),mix(mask[n.y*256u+q.x],mask[n.y*256u+n.x],t.x),t.y));
}
fn blur(pos:vec2<u32>,axis:vec2<f32>,kind:u32) -> vec4<f32> {
    let weights=array<f32,7>(0.064759,0.120985,0.176033,0.199471,0.176033,0.120985,0.064759);var color=vec4<f32>(0.);
    for(var i=0u;i<7u;i++) {let uv=(vec2<f32>(pos)+0.5+axis*(f32(i)-3.)*max(p.local.w/3.,1.))/vec2<f32>(p.dimensions.xy);color+=sample_pixel(uv,kind)*weights[i]/0.923025;}
    return vec4<f32>(color.xyz,1.);
}
@compute @workgroup_size(8,8) fn horizontal(@builtin(global_invocation_id) id:vec3<u32>) {if any(id.xy>=p.dimensions.xy) {return;}blurA[id.y*p.dimensions.x+id.x]=pack4x8unorm(blur(id.xy,vec2<f32>(1.,0.),0u));}
@compute @workgroup_size(8,8) fn vertical(@builtin(global_invocation_id) id:vec3<u32>) {if any(id.xy>=p.dimensions.xy) {return;}blurB[id.y*p.dimensions.x+id.x]=pack4x8unorm(blur(id.xy,vec2<f32>(0.,1.),1u));}
@compute @workgroup_size(8,8) fn effects(@builtin(global_invocation_id) id:vec3<u32>) {
    if any(id.xy>=p.dimensions.zw) {return;}
    let i=id.y*p.dimensions.z+id.x;var uv=((vec2<f32>(id.xy)+0.5)/vec2<f32>(p.dimensions.zw)-0.5)*p.aspect.xy/p.effects.xy+0.5;
    let radial=(uv-0.5)*2.;uv=0.5+radial*(1.+p.effects.z*dot(radial,radial))*0.5;
    let local=uv-p.local.xy;let distance=length(local)/p.local.z;
    if distance<1. {uv=p.local.xy+local*(1.-p.effects.w*(1.-distance)*(1.-distance));}
    if any(uv<vec2<f32>(0.)) || any(uv>vec2<f32>(1.)) {output[i]=0xff000000u;return;}
    if p.flags.y==1u {uv.x=1.-uv.x;}
    if p.flags.x==90u {uv=vec2<f32>(uv.y,1.-uv.x);}else if p.flags.x==180u {uv=1.-uv;}else if p.flags.x==270u {uv=vec2<f32>(1.-uv.y,uv.x);}
    var color=sample_pixel(uv,0u);
    if p.local.w>0. {color=mix(sample_pixel(uv,2u),color,mask_value(uv));}
    output[i]=pack4x8unorm(vec4<f32>(color.xyz,1.));
}
