#version 100
precision mediump float;

varying vec2 uv;
varying vec4 color;

uniform sampler2D Texture;
uniform float brightness;

void main() {
    vec4 tex = texture2D(Texture, uv);
    // MajdataPlay's `ColorAdjustEffect`: `_Brightness` scales the sampled RGB.
    // A float uniform (unlike macroquad's u8 vertex tint) may exceed 1.0, so a
    // break sprite can flash brighter than white on the pulse peak.
    gl_FragColor = vec4(tex.rgb * brightness, tex.a) * color;
}
