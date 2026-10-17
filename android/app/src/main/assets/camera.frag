#version 300 es
#extension GL_OES_EGL_image_external_essl3 : require
precision highp float;
uniform samplerExternalOES cameraTexture;
uniform sampler2D blurredTexture;
uniform sampler2D personMask;
uniform sampler2D guide;
uniform mat4 textureMatrix;
uniform vec2 aspectScale;
uniform vec2 stretch;
uniform vec2 center;
uniform float distortion;
uniform float bulge;
uniform float radius;
uniform float blurEnabled;
uniform float maskReady;
uniform int rotation;
uniform bool mirror;
uniform bool analysis;
uniform vec4 look; // hue radians, saturation, contrast, sharpen
uniform vec2 texel;
uniform vec2 maskTexel;
in vec2 uv;
out vec4 color;
vec3 fetch(vec2 point) { return texture(cameraTexture,(textureMatrix*vec4(point,0.0,1.0)).xy).rgb; }
// Luma-preserving hue rotation and saturation in YIQ, then contrast about mid grey.
vec3 grade(vec3 c) {
    const mat3 toYiq = mat3(0.299,0.596,0.211, 0.587,-0.274,-0.523, 0.114,-0.322,0.312);
    const mat3 toRgb = mat3(1.0,1.0,1.0, 0.956,-0.272,-1.106, 0.621,-0.647,1.703);
    vec3 yiq = toYiq*c;
    float cs = cos(look.x), sn = sin(look.x);
    yiq.yz = look.y*vec2(yiq.y*cs-yiq.z*sn, yiq.y*sn+yiq.z*cs);
    return clamp((toRgb*yiq-0.5)*look.z+0.5,0.0,1.0);
}
// Cross-bilateral matte: neighbouring mask samples count only when the full-resolution-ish guide colour matches,
// which snaps the person matte onto real colour edges instead of the model's blocky outline.
float matte(vec2 point) {
    vec3 centre = texture(guide,point).rgb;
    float sum = 0.0, total = 0.0;
    for (int y = -1; y <= 1; y++) for (int x = -1; x <= 1; x++) {
        vec2 at = point+vec2(float(x),float(y))*maskTexel*1.5;
        vec3 delta = texture(guide,at).rgb-centre;
        float weight = exp(-dot(delta,delta)*30.0)*(x == 0 && y == 0 ? 2.0 : 1.0);
        sum += texture(personMask,at).r*weight; total += weight;
    }
    return smoothstep(0.35,0.65,sum/total);
}
void main() {
    vec2 p = uv;
    if (analysis) p.y = 1.0-p.y;
    else {
        p = ((p-0.5)*aspectScale)/stretch+0.5;
        vec2 radial = (p-0.5)*2.0;
        p = 0.5+radial*(1.0+distortion*dot(radial,radial))*0.5;
        vec2 local = p-center;
        float distance = length(local)/radius;
        if (distance < 1.0) p = center+local*(1.0-bulge*pow(1.0-distance,2.0));
        if (any(lessThan(p,vec2(0.0))) || any(greaterThan(p,vec2(1.0)))) {
            color = vec4(0.0,0.0,0.0,1.0); return;
        }
    }
    vec2 samplePoint = p;
    if (mirror) samplePoint.x = 1.0-samplePoint.x;
    if (rotation == 90) samplePoint = vec2(1.0-samplePoint.y,samplePoint.x);
    else if (rotation == 180) samplePoint = 1.0-samplePoint;
    else if (rotation == 270) samplePoint = vec2(samplePoint.y,1.0-samplePoint.x);
    vec3 sharp = fetch(samplePoint);
    if (!analysis && look.w > 0.0) {
        vec3 around = (fetch(samplePoint+vec2(texel.x,0.0))+fetch(samplePoint-vec2(texel.x,0.0))+fetch(samplePoint+vec2(0.0,texel.y))+fetch(samplePoint-vec2(0.0,texel.y)))*0.25;
        sharp += look.w*(sharp-around);
    }
    if (!analysis && blurEnabled > 0.5) {
        vec2 maskPoint = vec2(p.x,1.0-p.y);
        float person = mix(1.0,matte(maskPoint),maskReady);
        sharp = mix(texture(blurredTexture,maskPoint).rgb,sharp,person);
    }
    color = vec4(analysis ? sharp : grade(clamp(sharp,0.0,1.0)),1.0);
}
