use memmap2::Mmap;
use std::fs::File;
use std::sync::Arc;
use std::thread;

const THREAD_N: u8 = 12;

#[derive(Copy, Clone)]
struct Aggregate {
    key: Option<[u8; 100]>,
    hash: u64,

    min: i32,
    max: i32,
    acc: i32,
    count: i32,
}

fn main() -> std::io::Result<()> {
    let file = File::open("measurements.txt")?;
    let mmap = Arc::new(unsafe { Mmap::map(&file)? });
    let mut threads_handles: Vec<thread::JoinHandle<Box<[Aggregate]>>> = Vec::new();

    let mut bounds: [u64; THREAD_N as usize + 1] = [0; THREAD_N as usize + 1];
    for thread_id in 1..THREAD_N + 1 {
        let right_bound_naive = mmap.len() as u64 / THREAD_N as u64 * thread_id as u64;
        bounds[thread_id as usize] = find_next_newline_pos(right_bound_naive, &mmap);
    }

    for thread_id in 1..THREAD_N + 1 {
        let left_bound = bounds[thread_id as usize - 1] as usize;
        let right_bound = bounds[thread_id as usize] as usize;

        let mmap_clone = Arc::clone(&mmap);
        threads_handles.push(thread::spawn(move || {
            process_lines(&mmap_clone[left_bound..right_bound])
        }));
    }

    let mut stations: Box<[Aggregate]> = vec![
        Aggregate {
            key: None,
            hash: 0,
            min: -1000,
            max: 1000,
            acc: 0,
            count: 0,
        };
        16384
    ]
    .into_boxed_slice();
    for handle in threads_handles {
        let joined_stations = handle.join().unwrap();
        stations = merge_stations(stations, joined_stations);
    }

    print_output(&stations);
    Ok(())
}

fn find_next_newline_pos(mut current_pos: u64, mmap: &Mmap) -> u64 {
    if current_pos as usize == mmap.len() {
        return current_pos;
    }
    loop {
        if *mmap.get(current_pos as usize).unwrap() == b'\n' {
            return current_pos + 1;
        }
        current_pos += 1;
    }
}

fn process_lines(mmap: &[u8]) -> Box<[Aggregate]> {
    let mut stations: Box<[Aggregate]> = vec![
        Aggregate {
            key: None,
            hash: 0,
            min: -1000,
            max: 1000,
            acc: 0,
            count: 0,
        };
        16384
    ]
    .into_boxed_slice();

    let mut station_acc: [u8; 100] = [59; 100];
    let mut station_cnt: u8 = 0;
    let mut station_hsh: u64 = 5381;
    let mut station_flg: bool = false;

    let mut float_acc: i32 = 0;
    let mut float_sign: i8 = 1;

    for i in 0..mmap.len() {
        let b = &mmap[i];

        if *b == b'\n' {
            float_acc *= float_sign as i32;
            stations = process_line(station_acc, station_hsh, float_acc, stations);

            station_acc = [59; 100];
            station_cnt = 0;
            station_hsh = 5381;
            station_flg = false;

            float_acc = 0;
            float_sign = 1;
        } else if *b == b';' {
            station_flg = true;
        } else {
            if !station_flg {
                station_acc[station_cnt as usize] = *b;
                station_cnt += 1;
                station_hsh = ((station_hsh << 5) + station_hsh) + *b as u64;
            } else if *b == b'-' {
                float_sign = -1;
            } else if *b == b'.' {
                continue;
            } else {
                float_acc *= 10;
                float_acc += *b as i32 - 48;
            }
        }
    }

    stations
}

fn process_line(
    station: [u8; 100],
    station_hsh: u64,
    temperature: i32,
    mut stations: Box<[Aggregate]>,
) -> Box<[Aggregate]> {
    let mut station_ind = station_hsh as usize;
    loop {
        station_ind &= 16384 - 1;
        if stations[station_ind].key.is_none() {
            stations[station_ind] = Aggregate {
                key: Some(station),
                hash: station_hsh,
                min: temperature,
                max: temperature,
                acc: temperature,
                count: 1,
            };
            break;
        } else if stations[station_ind].key == Some(station) {
            let current_agg = &mut stations[station_ind];
            current_agg.acc += temperature;
            current_agg.count += 1;
            if current_agg.min > temperature {
                current_agg.min = temperature;
            }
            if current_agg.max < temperature {
                current_agg.max = temperature;
            }
            break;
        } else {
            station_ind += 1;
        }
    }

    stations
}

fn merge_stations(
    mut stations1: Box<[Aggregate]>,
    stations2: Box<[Aggregate]>,
) -> Box<[Aggregate]> {
    for station in stations2.iter() {
        if station.key.is_some() {
            let mut station_ind = station.hash as usize;
            loop {
                station_ind &= 16384 - 1;
                if stations1[station_ind].key.is_none() {
                    stations1[station_ind] = *station;
                    break;
                } else if stations1[station_ind].key == station.key {
                    let current_agg = &mut stations1[station_ind];
                    current_agg.acc += station.acc;
                    current_agg.count += station.count;
                    if current_agg.min > station.min {
                        current_agg.min = station.min;
                    }
                    if current_agg.max < station.max {
                        current_agg.max = station.max;
                    }
                    break;
                } else {
                    station_ind += 1;
                }
            }
        }
    }
    stations1
}

fn print_output(stations: &[Aggregate]) {
    print!("{{");
    let mut output_vec: Vec<&Aggregate> = stations.iter().collect();

    let mut is_first = true;
    output_vec.sort_by(|a, b| a.key.cmp(&b.key));
    for station in output_vec {
        if station.key.is_none() {
            continue;
        }

        if !is_first {
            print!(", ");
        } else {
            is_first = false;
        }

        let mn = station.min as f32 / 10.0;
        let mx = station.max as f32 / 10.0;
        let mean = station.acc as f32 / station.count as f32 / 10.0;

        let mut right_bound = 100;
        let station_key = station.key.unwrap();
        for i in 0..station_key.len() {
            if station_key[i] == b';' {
                right_bound = i;
                break;
            }
        }
        let station = unsafe { std::str::from_utf8_unchecked(&station_key[..right_bound]) };
        print!("{station}={mn:.1}/{mean:.1}/{mx:.1}");
    }
    print!("}}");
}
