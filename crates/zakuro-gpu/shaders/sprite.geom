#version 450

// passes each triangle on as it is, w/ its texture coord set 0 rectangle for raster.frag when it's flat 2D (w of 1
// at every corner), so the fragments by a sprite's edges keep set 0 half a texel inside it, like the PICA's do at
// the native size; upscaled, filtering would blend in the texels past the edge as a sliver
// the vertex stage before it is raster.vert, shade.vert or a translated program, they all hand over the same outputs

layout(triangles) in;
layout(triangle_strip, max_vertices = 3) out;

layout(location = 0) in vec4 in_color[];
layout(location = 1) in vec4 in_texcoords01[];
layout(location = 2) in vec2 in_texcoord2[];
layout(location = 3) noperspective in float in_depth[];
layout(location = 4) in vec4 in_quaternion[];
layout(location = 5) in vec4 in_view[];

layout(location = 0) out vec4 out_color;
layout(location = 1) out vec4 out_texcoords01;
layout(location = 2) out vec2 out_texcoord2;
layout(location = 3) noperspective out float out_depth;
layout(location = 4) out vec4 out_quaternion;
layout(location = 5) out vec4 out_view;
// min then max, min past max when it isn't flat
layout(location = 6) flat out vec4 out_bounds;

void main() {
    bool flat2d = gl_in[0].gl_Position.w == 1.0 && gl_in[1].gl_Position.w == 1.0 && gl_in[2].gl_Position.w == 1.0;
    vec2 a = in_texcoords01[0].xy, b = in_texcoords01[1].xy, c = in_texcoords01[2].xy;
    vec4 bounds = flat2d ? vec4(min(min(a, b), c), max(max(a, b), c)) : vec4(1.0, 1.0, 0.0, 0.0);
    for (int i = 0; i < 3; i++) {
        gl_Position = gl_in[i].gl_Position;
        out_color = in_color[i];
        out_texcoords01 = in_texcoords01[i];
        out_texcoord2 = in_texcoord2[i];
        out_depth = in_depth[i];
        out_quaternion = in_quaternion[i];
        out_view = in_view[i];
        out_bounds = bounds;
        EmitVertex();
    }
    EndPrimitive();
}
