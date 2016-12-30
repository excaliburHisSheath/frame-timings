#![feature(test)]

extern crate clap;
// extern crate stopwatch;
extern crate test;

use clap::{Arg, App};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
// use std::fs::File;
// use std::io::Write;
use std::num::Wrapping;
use std::thread;
use std::time::*;
// use stopwatch::Stopwatch;

type TestRoutine = fn(TestParams) -> Vec<Duration>;

fn main() {
    let matches = App::new("Frame Timing Test Suite")
        .version("1.0")
        .author("David LeGare <excaliburhissheath@gmail.com>")
        .about("A test suite for comparing different styles of game loop")
        .arg(Arg::with_name("test name").takes_value(true))
        .arg(
            Arg::with_name("num frames")
            .short("f")
            .long("frames")
            .takes_value(true)
        )
        .get_matches();

    let tests = {
        let mut tests = HashMap::<_, TestRoutine>::new();
        tests.insert("test_0", loop_0);
        tests.insert("test_1", loop_1);
        tests.insert("test_2", loop_2);
        tests.insert("test_3", loop_3);
        tests
    };

    // TODO: Allow these to be passed in as arguments.
    let params = TestParams {
        target_frame_time: Duration::new(0, 16_666_667),
        frames_to_simulate: 1 * 60,
        workload: 10_000_000,
    };

    if let Some(test_name) = matches.value_of("test name") {
        // Test only the specified routine.
        if let Some(routine) = tests.get(test_name) {
            let results = run_test(*routine, params);
            println!("params: {:#?}", params);
            println!("{} results: {:#?}", test_name, results);
        } else {
            // TODO: Use clap's configuration to remove this possiblity.
            println!("Unrecognized test name: \"{}\"", test_name);
        }
    } else {
        // Test all the routines.
        println!("params: {:#?}", params);

        for (test_name, routine) in tests {
            let results = run_test(routine, params);
            println!("{} results: {:#?}", test_name, results);
        }
    }

    // let file_name = format!("{}_{}_frames.json", test_name, frames_to_simulate);
    //
    // // Write out stopwatch data.
    // let events_string = stopwatch::write_events_to_string();
    // let mut out_file = File::create(&*file_name).unwrap();
    // out_file.write_all(events_string.as_bytes()).unwrap();
}

#[derive(Debug, Clone, Copy)]
struct TestParams {
    target_frame_time: Duration,
    frames_to_simulate: usize,
    workload: usize
}

#[derive(Debug, Clone, Copy)]
struct TestResults {
    min: Duration,
    max: Duration,
    mean: Duration,
    std: Duration,
    long_frames: usize,
}

fn as_nanos(duration: Duration) -> u64 {
    duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64
}

fn from_nanos(nanos: u64) -> Duration {
    let secs = nanos / 1_000_000_000;
    let subsec_nanos = nanos % 1_000_000_000;
    Duration::new(secs, subsec_nanos as u32)
}

fn run_test(test_routine: TestRoutine, params: TestParams) -> TestResults {
    let times = test_routine(params);

    let mut min = times[0];
    let mut max = times[0];
    let mut total = Duration::new(0, 0);
    let mut long_frames = 0;

    for time in times.iter().cloned() {
        total += time;
        if time < min { min = time; }
        if time > max { max = time; }
        if time > params.target_frame_time { long_frames += 1; }
    }

    let mean = total / params.frames_to_simulate as u32;
    let total_sqr_deviation = times.into_iter().fold(0, |total, time| {
        let diff = if time < mean { mean - time } else { time - mean };

        // Convert to nanos so that we can square and hope we don't overflow ¯\_(ツ)_/¯.
        let nanos = as_nanos(diff);
        let diff_sqr = nanos * nanos;

        total + diff_sqr
    });

    let std_dev = f64::sqrt(total_sqr_deviation as f64 / params.frames_to_simulate as f64);

    TestResults {
        min: min,
        max: max,
        mean: mean,
        std: from_nanos(std_dev as u64),
        long_frames: long_frames,
    }
}

/// Performs a deterministic amount of work, returning the duration the work took.
///
///
pub fn do_work(iterations: usize) -> Duration {
    // let _s = Stopwatch::new("work");

    let start_time = Instant::now();

    let mut prev = Wrapping(0);
    let mut current = Wrapping(1);
    for _ in 0..iterations {
        let temp = current;
        current = prev + current;
        prev = temp;
    }
    test::black_box(current);

    start_time.elapsed()
}

fn loop_0(TestParams { target_frame_time, frames_to_simulate, workload }: TestParams) -> Vec<Duration> {
    let mut times = Vec::with_capacity(frames_to_simulate);

    for _ in 0..frames_to_simulate {
        let frame_start = Instant::now();

        // Simulate the workload.
        let duration = do_work(workload);
        times.push(duration);

        // If the frame took too long the subtraction will overflow, so we have to check first.
        let mut elapsed_time = frame_start.elapsed();
        if elapsed_time < target_frame_time {
            let mut remaining_time = target_frame_time - elapsed_time;

            // Sleep the thread to kill time.
            while remaining_time > Duration::from_millis(1) {
                thread::sleep(remaining_time);

                // Check again if we've passed the frame time to avoid overflow in the subtraction.
                elapsed_time = frame_start.elapsed();
                if elapsed_time < target_frame_time {
                    remaining_time = target_frame_time - elapsed_time;
                } else {
                    break;
                }
            }

            // Not enough time to sleep the thread,
            // just spin until we reach the target time.
            while frame_start.elapsed() < target_frame_time {}
        }
    }

    times
}

fn loop_1(TestParams { target_frame_time, frames_to_simulate, workload }: TestParams) -> Vec<Duration> {
    let mut times = Vec::with_capacity(frames_to_simulate);

    let mut frame_start = Instant::now();

    for _ in 0..frames_to_simulate {
        // Simulate the workload.
        let duration = do_work(workload);
        times.push(duration);

        // Move the frame start time up by the frame length.
        frame_start += target_frame_time;

        // If the frame took too long the subtraction will overflow, so we have to check first.
        let mut now = Instant::now();
        if now < frame_start {
            let mut remaining_time = frame_start - now;

            // Sleep the thread to kill time.
            while remaining_time > Duration::from_millis(1) {
                thread::sleep(remaining_time);

                // Check again if we've passed the frame time to avoid overflow in the subtraction.
                now = Instant::now();
                if now < frame_start {
                    remaining_time = frame_start - now;
                } else {
                    break;
                }
            }

            // Not enough time to sleep the thread,
            // just spin until we reach the target time.
            while Instant::now() < frame_start {}
        }
    }

    times
}

fn loop_2(TestParams { target_frame_time, frames_to_simulate, workload }: TestParams) -> Vec<Duration> {
    let mut times = Vec::with_capacity(frames_to_simulate);

    let mut last_frame_time = Instant::now();
    let mut remaining_update_time = Duration::new(0, 0);

    let mut loops_done = 0;

    while loops_done < frames_to_simulate {
        let frame_start = Instant::now();
        remaining_update_time += frame_start - last_frame_time;

        while remaining_update_time > target_frame_time {
            // Simulate the workload.
            let duration = do_work(workload);
            times.push(duration);

            remaining_update_time -= target_frame_time;
            loops_done += 1;
        }

        last_frame_time = frame_start;

        thread::sleep(Duration::new(0, 0));
    }

    times
}

fn loop_3(TestParams { target_frame_time, frames_to_simulate, workload }: TestParams) -> Vec<Duration> {
    let mut times = Vec::with_capacity(frames_to_simulate);

    let mut frame_start = Instant::now();
    for _ in 0..frames_to_simulate {
        // Simulate the workload.
        let duration = do_work(workload);
        times.push(duration);

        // Determine when the next frame should start, accounting for the case that we missed our
        // frame time and might need to drop frames.
        while frame_start < Instant::now() {
            frame_start += target_frame_time;
        }

        // Now wait until we've returned to the frame cadence before beginning the next frame.
        while Instant::now() < frame_start {
            thread::sleep(Duration::new(0, 0));
        }
    }

    times
}

/// Helper struct for pretty-printing durations of time.
///
/// Wraps a `Duration` object to provide a `Display` implementation. The formatting shows the time
/// in miliseconds with microsecond precision after the decimal (3 digits after the decimal).
pub struct PrettyDuration(Duration);

impl Display for PrettyDuration {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        let nanos_total = self.0.subsec_nanos();
        let millis = nanos_total / 1_000_000;
        let micros = (nanos_total % 1_000_000) / 1_000;

        write!(formatter, "{}.{}ms", millis, micros)
    }
}
