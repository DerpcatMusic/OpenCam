#version 300 es
precision mediump float;
uniform sampler2D previous;
uniform sampler2D latest;
uniform float blend;
in vec2 uv;
out vec4 color;
void main() { color = vec4(mix(texture(previous,uv).r,texture(latest,uv).r,blend),0.0,0.0,1.0); }
