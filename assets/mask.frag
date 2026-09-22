#version 330

in vec2 uv;
out vec4 FragColor;

uniform sampler2D Texture;
uniform float progress;

const float PI = 3.14159265359;

void main() {
    vec4 color = texture(Texture, uv);

    vec2 center = vec2(0.5, 0.5);
    vec2 dir = uv - center;

    // atan returns [-PI, PI]
    float angle = atan(dir.y, dir.x);

    // start from 12 o'clock
    angle += PI / 2.0;

    if (angle < 0.0) {
        angle += PI * 2.0;
    }

    float normalizedAngle = angle / (PI * 2.0);

    // smooth transition at the progress edge
    float diff = normalizedAngle - progress;
    float alpha = 1.0 - smoothstep(0.0, 0.03, diff);

    FragColor = vec4(color.rgb, color.a * alpha);
}
