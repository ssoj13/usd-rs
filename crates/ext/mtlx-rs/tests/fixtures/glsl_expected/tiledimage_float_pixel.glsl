#version 400

struct BSDF { vec3 response; vec3 throughput; };
#define EDF vec3
struct displacementshader { vec3 offset; float scale; };
struct lightshader { vec3 intensity; vec3 direction; };
#define material surfaceshader
struct surfaceshader { vec3 color; vec3 transparency; };
struct volumeshader { vec3 color; vec3 transparency; };

// Uniform block: PublicUniforms
uniform sampler2D file;
uniform float default1;
uniform vec2 uvtiling;
uniform vec2 uvoffset;
uniform vec2 realworldimagesize;
uniform vec2 realworldtilesize;
uniform int filtertype;
uniform int framerange;
uniform int frameoffset;
uniform int frameendaction;
uniform int N_img_float_layer;
uniform int N_img_float_uaddressmode;
uniform int N_img_float_vaddressmode;
uniform int geomprop_UV0_index;

in VertexData
{
    vec2 texcoord_0;
} vd;

// Pixel shader outputs
out vec4 out1;

#define M_FLOAT_EPS 1e-8

#define mx_mod mod
#define mx_inverse inverse
#define mx_inversesqrt inversesqrt
#define mx_sin sin
#define mx_cos cos
#define mx_tan tan
#define mx_asin asin
#define mx_acos acos
#define mx_atan atan
#define mx_radians radians
#define mx_float_bits_to_int floatBitsToInt

vec2 mx_matrix_mul(vec2 v, mat2 m) { return v * m; }
vec3 mx_matrix_mul(vec3 v, mat3 m) { return v * m; }
vec4 mx_matrix_mul(vec4 v, mat4 m) { return v * m; }
vec2 mx_matrix_mul(mat2 m, vec2 v) { return m * v; }
vec3 mx_matrix_mul(mat3 m, vec3 v) { return m * v; }
vec4 mx_matrix_mul(mat4 m, vec4 v) { return m * v; }
mat2 mx_matrix_mul(mat2 m1, mat2 m2) { return m1 * m2; }
mat3 mx_matrix_mul(mat3 m1, mat3 m2) { return m1 * m2; }
mat4 mx_matrix_mul(mat4 m1, mat4 m2) { return m1 * m2; }

float mx_square(float x)
{
    return x*x;
}

vec2 mx_square(vec2 x)
{
    return x*x;
}

vec3 mx_square(vec3 x)
{
    return x*x;
}

vec3 mx_srgb_encode(vec3 color)
{
    bvec3 isAbove = greaterThan(color, vec3(0.0031308));
    vec3 linSeg = color * 12.92;
    vec3 powSeg = 1.055 * pow(max(color, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
    return mix(linSeg, powSeg, isAbove);
}


#define AIRY_FRESNEL_ITERATIONS 2

vec2 mx_transform_uv(vec2 uv, vec2 uv_scale, vec2 uv_offset)
{
    uv = uv * uv_scale + uv_offset;
    return uv;
}

void mx_image_float(sampler2D tex_sampler, int layer, float defaultval, vec2 texcoord, int uaddressmode, int vaddressmode, int filtertype, int framerange, int frameoffset, int frameendaction, vec2 uv_scale, vec2 uv_offset, out float result)
{
    vec2 uv = mx_transform_uv(texcoord, uv_scale, uv_offset);
    result = texture(tex_sampler, uv).r;
}

void main()
{
vec2 geomprop_UV0_out1 = vd.texcoord_0.xy;
vec2 N_mult_float_out = geomprop_UV0_out1 * uvtiling;
vec2 N_sub_float_out = N_mult_float_out - uvoffset;
vec2 N_divtilesize_float_out = N_sub_float_out / realworldimagesize;
vec2 N_multtilesize_float_out = N_divtilesize_float_out * realworldtilesize;
float N_img_float_out = 0.0;
mx_image_float(file, N_img_float_layer, default1, N_multtilesize_float_out, N_img_float_uaddressmode, N_img_float_vaddressmode, filtertype, framerange, frameoffset, frameendaction, vec2(1,1), vec2(0,0), N_img_float_out);
    out1 = vec4(N_img_float_out, 0.0, 0.0, 1.0);
}
