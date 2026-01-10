//! RustKit browser engine benchmarks
//!
//! Run with: cargo bench -p rustkit-bench

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rustkit_css::Stylesheet;
use rustkit_dom::Document;
use rustkit_layout::{BoxType, Dimensions, LayoutBox, Rect};

fn html_parsing_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_parsing");

    // Small document
    let small =
        r#"<!DOCTYPE html><html><head><title>Test</title></head><body><p>Hello</p></body></html>"#;
    group.throughput(Throughput::Bytes(small.len() as u64));
    group.bench_with_input(BenchmarkId::new("parse", "small"), small, |b, html| {
        b.iter(|| Document::parse_html(html))
    });

    // Medium document (100 paragraphs)
    let medium = generate_html(100);
    group.throughput(Throughput::Bytes(medium.len() as u64));
    group.bench_with_input(BenchmarkId::new("parse", "medium"), &medium, |b, html| {
        b.iter(|| Document::parse_html(html))
    });

    // Large document (1000 paragraphs)
    let large = generate_html(1000);
    group.throughput(Throughput::Bytes(large.len() as u64));
    group.bench_with_input(BenchmarkId::new("parse", "large"), &large, |b, html| {
        b.iter(|| Document::parse_html(html))
    });

    group.finish();
}

fn css_parsing_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("css_parsing");

    // Small stylesheet
    let small = "body { margin: 0; } h1 { font-size: 24px; }";
    group.throughput(Throughput::Bytes(small.len() as u64));
    group.bench_with_input(BenchmarkId::new("parse", "small"), small, |b, css| {
        b.iter(|| Stylesheet::parse(css))
    });

    // Medium stylesheet (50 rules)
    let medium = generate_css(50);
    group.throughput(Throughput::Bytes(medium.len() as u64));
    group.bench_with_input(BenchmarkId::new("parse", "medium"), &medium, |b, css| {
        b.iter(|| Stylesheet::parse(css))
    });

    // Large stylesheet (200 rules)
    let large = generate_css(200);
    group.throughput(Throughput::Bytes(large.len() as u64));
    group.bench_with_input(BenchmarkId::new("parse", "large"), &large, |b, css| {
        b.iter(|| Stylesheet::parse(css))
    });

    group.finish();
}

fn layout_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout");

    group.bench_function("simple", |b| {
        let style = rustkit_css::ComputedStyle::new();
        let mut containing = Dimensions::default();
        containing.content = Rect::new(0.0, 0.0, 800.0, 600.0);

        b.iter(|| {
            let mut root = LayoutBox::new(BoxType::Block, style.clone());
            root.layout(&containing);
        })
    });

    group.bench_function("nested_10", |b| {
        let style = rustkit_css::ComputedStyle::new();
        let mut containing = Dimensions::default();
        containing.content = Rect::new(0.0, 0.0, 800.0, 600.0);

        b.iter(|| {
            let mut root = LayoutBox::new(BoxType::Block, style.clone());
            for _ in 0..10 {
                root.children
                    .push(LayoutBox::new(BoxType::Block, style.clone()));
            }
            root.layout(&containing);
        })
    });

    group.bench_function("nested_100", |b| {
        let style = rustkit_css::ComputedStyle::new();
        let mut containing = Dimensions::default();
        containing.content = Rect::new(0.0, 0.0, 800.0, 600.0);

        b.iter(|| {
            let mut root = LayoutBox::new(BoxType::Block, style.clone());
            for _ in 0..100 {
                root.children
                    .push(LayoutBox::new(BoxType::Block, style.clone()));
            }
            root.layout(&containing);
        })
    });

    group.finish();
}

fn generate_html(n: usize) -> String {
    let mut html = String::from("<!DOCTYPE html><html><head><title>Test</title></head><body>");
    for i in 0..n {
        html.push_str(&format!("<p>Paragraph number {}</p>", i));
    }
    html.push_str("</body></html>");
    html
}

fn generate_css(n: usize) -> String {
    let mut css = String::new();
    for i in 0..n {
        css.push_str(&format!(
            ".class{} {{ margin: {}px; padding: {}px; color: red; }}\n",
            i,
            i,
            i * 2
        ));
    }
    css
}

criterion_group!(
    benches,
    html_parsing_benchmarks,
    css_parsing_benchmarks,
    layout_benchmarks,
);

criterion_main!(benches);
