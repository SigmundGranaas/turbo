//! Run a scripted session against the synthetic world and dump artifacts:
//! keyframe PNGs + a metrics JSON. The visual counterpart of the session
//! tests — lets a human (or an agent) *watch* what the assertions check.
//!
//! Usage: journey [--zoom-from Z] [--zoom-to Z] [--latency N]
//!                [--size WxH] [--out-dir DIR]

use std::time::Duration;

use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, MapEngine};
use turbomap_sim::{basemap_scene, PerfSummary, Sim};

struct Args {
    zoom_from: f64,
    zoom_to: f64,
    latency: u64,
    width: u32,
    height: u32,
    out_dir: String,
}

fn parse() -> Args {
    let mut a = Args {
        zoom_from: 11.0,
        zoom_to: 13.0,
        latency: 3,
        width: 640,
        height: 400,
        out_dir: "/tmp/turbomap-journey".to_string(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--zoom-from" => a.zoom_from = it.next().unwrap().parse().unwrap(),
            "--zoom-to" => a.zoom_to = it.next().unwrap().parse().unwrap(),
            "--latency" => a.latency = it.next().unwrap().parse().unwrap(),
            "--size" => {
                let v = it.next().expect("--size WxH");
                let (w, h) = v.split_once('x').expect("WxH");
                a.width = w.parse().unwrap();
                a.height = h.parse().unwrap();
            }
            "--out-dir" => a.out_dir = it.next().unwrap(),
            other => panic!("unknown arg: {other}"),
        }
    }
    a
}

fn save(sim: &Sim, dir: &str, name: &str) {
    if let Some(img) = &sim.last {
        let path = format!("{dir}/{name}.png");
        img.save(&path).expect("save keyframe");
        println!("wrote {path}");
    }
}

fn main() {
    let args = parse();
    std::fs::create_dir_all(&args.out_dir).expect("out dir");

    let mut sim = Sim::new(
        args.width,
        args.height,
        Sim::start_camera(args.zoom_from),
        MapOptions::default(),
    )
    .expect("no wgpu adapter (install mesa-vulkan-drivers)");

    sim.engine.apply(basemap_scene());
    sim.run_until_stable(300, 0.002)
        .expect("initial load settles");
    save(&sim, &args.out_dir, "01-loaded");

    sim.latency_frames = args.latency;
    sim.engine.ease_to(
        CameraState {
            zoom: args.zoom_to,
            ..sim.camera()
        },
        Duration::from_millis(800),
    );
    let mut mid_saved = false;
    for _ in 0..600 {
        let s = sim.step();
        let s_animating = s.animating;
        let s_zoom = s.zoom;
        let s_in_flight = s.in_flight;
        if !mid_saved
            && (s_zoom - args.zoom_from).abs() > (args.zoom_to - args.zoom_from).abs() / 2.0
        {
            save(&sim, &args.out_dir, "02-mid-zoom");
            mid_saved = true;
        }
        if !s_animating && s_in_flight == 0 {
            break;
        }
    }
    sim.run_until_stable(300, 0.002).expect("post-zoom settles");
    save(&sim, &args.out_dir, "03-arrived");

    let summary = PerfSummary::from_stats(&sim.stats);
    let json = serde_json::to_string_pretty(&summary).unwrap();
    std::fs::write(format!("{}/metrics.json", args.out_dir), &json).unwrap();
    println!("{json}");
}
