#version 100
precision mediump float;

varying vec2 uv;
uniform sampler2D Texture;
uniform float progress;

void main() {
    vec2 center = vec2(0.5, 0.5);
    float a = atan(uv.y - center.y, uv.x - center.x);
    float clock_angle = mod(a + 1.5708, 6.28318);
    float sweep = progress * 6.28318;
    float visible = step(clock_angle, sweep);
    vec4 color = texture2D(Texture, uv);
    gl_FragColor = vec4(color.rgb, color.a * visible);
}
