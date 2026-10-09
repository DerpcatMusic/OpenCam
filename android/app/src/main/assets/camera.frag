#version 300 es
#extension GL_OES_EGL_image_external_essl3 : require
precision highp float;
uniform samplerExternalOES cameraTexture;
uniform sampler2D blurredTexture;
uniform sampler2D personMask;
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
in vec2 uv;
out vec4 color;
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
    vec3 sharp = texture(cameraTexture,(textureMatrix*vec4(samplePoint,0.0,1.0)).xy).rgb;
    if (!analysis && blurEnabled > 0.5) {
        vec2 maskPoint = vec2(p.x,1.0-p.y);
        float person = maskReady*smoothstep(0.25,0.75,texture(personMask,maskPoint).r);
        sharp = mix(texture(blurredTexture,maskPoint).rgb,sharp,person);
    }
    color = vec4(sharp,1.0);
}
