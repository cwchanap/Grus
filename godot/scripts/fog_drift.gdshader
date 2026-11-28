// Low blizzard fog drifting sideways across a painted backdrop: two layers of
// procedural noise scrolling at different speeds, densest at the bottom of
// the rect and gone by its upper third. Presentation only (weather.tscn).
shader_type canvas_item;

uniform vec4 tint : source_color = vec4(0.8, 0.87, 0.94, 1.0);
uniform float strength = 0.32;
uniform float speed = 0.02;
uniform float scale = 3.0;

float hash(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	vec2 u = f * f * (3.0 - 2.0 * f);
	return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
		mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

float fbm(vec2 p) {
	float value = 0.0;
	float amplitude = 0.5;
	for (int octave = 0; octave < 4; octave++) {
		value += amplitude * noise(p);
		p *= 2.03;
		amplitude *= 0.5;
	}
	return value;
}

void fragment() {
	vec2 p = UV * vec2(scale * 1.78, scale);
	float t = TIME * speed;
	float far = fbm(p + vec2(t * 4.0, 0.0));
	float near = fbm(p * 1.6 + vec2(t * 9.0, t * 0.8));
	float cloud = smoothstep(0.38, 0.85, far * 0.55 + near * 0.45);
	float band = smoothstep(0.3, 1.0, UV.y);
	COLOR = vec4(tint.rgb, cloud * band * strength);
}
