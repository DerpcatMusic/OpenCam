#version 300 es
precision highp float;
uniform sampler2D image;
uniform vec2 stepSize;
in vec2 uv;
out vec4 color;
void main() {
    vec3 sum = texture(image,uv).rgb*0.227027;
    sum += (texture(image,uv+stepSize*1.384615).rgb+texture(image,uv-stepSize*1.384615).rgb)*0.316216;
    sum += (texture(image,uv+stepSize*3.230769).rgb+texture(image,uv-stepSize*3.230769).rgb)*0.070270;
    color = vec4(sum,1.0);
}
