//! Benchmarks comparing interpreter vs JIT vs batched shader execution.
//!
//! Run with: `cargo bench --bench shader_exec`

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use osl_rs::batched::{BatchedShaderGlobals, NullBatchedRenderer};
use osl_rs::batched_exec::BatchedInterpreter;
use osl_rs::codegen;
use osl_rs::interp::Interpreter;
use osl_rs::math::Vec3;
use osl_rs::parser;
use osl_rs::shaderglobals::ShaderGlobals;

#[cfg(feature = "jit")]
use osl_rs::jit::{CraneliftBackend, JitBackend};

// ---------------------------------------------------------------------------
// Shader sources
// ---------------------------------------------------------------------------

const SHADER_SIMPLE: &str = r#"
shader simple(float Kd = 0.5, output color result = 0) {
    result = Kd * color(N.x, N.y, N.z);
}
"#;

const SHADER_NOISE: &str = r#"
shader noisy(float scale = 4.0, output float result = 0) {
    result = noise("perlin", P * scale);
}
"#;

const SHADER_MATH_HEAVY: &str = r#"
shader math_heavy(output float result = 0) {
    float x = u * 6.28318;
    float y = v * 6.28318;
    result = sin(x) * cos(y) + sqrt(abs(sin(x * 2.0))) + pow(abs(cos(y)), 2.2);
}
"#;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_globals() -> ShaderGlobals {
    let mut sg = ShaderGlobals::default();
    sg.p = Vec3::new(1.5, 2.5, 3.5);
    sg.n = Vec3::new(0.0, 1.0, 0.0);
    sg.i = Vec3::new(0.0, 0.0, -1.0);
    sg.u = 0.5;
    sg.v = 0.5;
    sg
}

fn make_batched_globals<const W: usize>() -> BatchedShaderGlobals<W> {
    let mut bg = BatchedShaderGlobals::default();
    for i in 0..W {
        let t = i as f32 / W as f32;
        bg.p.data[i] = Vec3::new(1.5 + t, 2.5, 3.5);
        bg.n.data[i] = Vec3::new(0.0, 1.0, 0.0);
        bg.i.data[i] = Vec3::new(0.0, 0.0, -1.0);
        bg.u.data[i] = 0.5 + t * 0.1;
        bg.v.data[i] = 0.5;
    }
    bg
}

fn compile_ir(src: &str) -> codegen::ShaderIR {
    let ast = parser::parse(src).unwrap().ast;
    codegen::generate(&ast)
}

// ---------------------------------------------------------------------------
// Interpreter benchmarks
// ---------------------------------------------------------------------------

fn bench_interpreter(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter");

    let ir_simple = compile_ir(SHADER_SIMPLE);
    let ir_noise = compile_ir(SHADER_NOISE);
    let ir_math = compile_ir(SHADER_MATH_HEAVY);
    let globals = make_globals();

    group.bench_function("simple", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.execute(black_box(&ir_simple), black_box(&globals), None);
        })
    });

    group.bench_function("noise", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.execute(black_box(&ir_noise), black_box(&globals), None);
        })
    });

    group.bench_function("math_heavy", |b| {
        b.iter(|| {
            let mut interp = Interpreter::new();
            interp.execute(black_box(&ir_math), black_box(&globals), None);
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// JIT benchmarks
// ---------------------------------------------------------------------------

#[cfg(feature = "jit")]
fn bench_jit(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit");

    let ir_simple = compile_ir(SHADER_SIMPLE);
    let ir_noise = compile_ir(SHADER_NOISE);
    let ir_math = compile_ir(SHADER_MATH_HEAVY);
    let mut globals = make_globals();

    let backend = CraneliftBackend::new();

    // Compile once (not part of the benchmark)
    let compiled_simple = backend.compile(&ir_simple).unwrap();
    let compiled_noise = backend.compile(&ir_noise).unwrap();
    let compiled_math = backend.compile(&ir_math).unwrap();

    group.bench_function("simple", |b| {
        b.iter(|| {
            compiled_simple.execute(black_box(&mut globals));
        })
    });

    group.bench_function("noise", |b| {
        b.iter(|| {
            compiled_noise.execute(black_box(&mut globals));
        })
    });

    group.bench_function("math_heavy", |b| {
        b.iter(|| {
            compiled_math.execute(black_box(&mut globals));
        })
    });

    group.finish();
}

#[cfg(feature = "jit")]
fn bench_jit_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compile");

    let ir_simple = compile_ir(SHADER_SIMPLE);
    let ir_noise = compile_ir(SHADER_NOISE);
    let ir_math = compile_ir(SHADER_MATH_HEAVY);

    group.bench_function("simple", |b| {
        b.iter(|| {
            let backend = CraneliftBackend::new();
            let _compiled = backend.compile(black_box(&ir_simple)).unwrap();
        })
    });

    group.bench_function("noise", |b| {
        b.iter(|| {
            let backend = CraneliftBackend::new();
            let _compiled = backend.compile(black_box(&ir_noise)).unwrap();
        })
    });

    group.bench_function("math_heavy", |b| {
        b.iter(|| {
            let backend = CraneliftBackend::new();
            let _compiled = backend.compile(black_box(&ir_math)).unwrap();
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Batched benchmarks (8-wide)
// ---------------------------------------------------------------------------

fn bench_batched(c: &mut Criterion) {
    let mut group = c.benchmark_group("batched_8");

    let ir_simple = compile_ir(SHADER_SIMPLE);
    let ir_noise = compile_ir(SHADER_NOISE);
    let ir_math = compile_ir(SHADER_MATH_HEAVY);
    let renderer = NullBatchedRenderer;

    group.bench_function("simple", |b| {
        b.iter(|| {
            let globals = make_batched_globals::<8>();
            let mut exec = BatchedInterpreter::<8>::new();
            exec.execute(black_box(&ir_simple), black_box(&globals), &renderer);
        })
    });

    group.bench_function("noise", |b| {
        b.iter(|| {
            let globals = make_batched_globals::<8>();
            let mut exec = BatchedInterpreter::<8>::new();
            exec.execute(black_box(&ir_noise), black_box(&globals), &renderer);
        })
    });

    group.bench_function("math_heavy", |b| {
        b.iter(|| {
            let globals = make_batched_globals::<8>();
            let mut exec = BatchedInterpreter::<8>::new();
            exec.execute(black_box(&ir_math), black_box(&globals), &renderer);
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg(feature = "jit")]
criterion_group!(
    benches,
    bench_interpreter,
    bench_jit,
    bench_jit_compile,
    bench_batched
);

#[cfg(not(feature = "jit"))]
criterion_group!(benches, bench_interpreter, bench_batched);

criterion_main!(benches);
