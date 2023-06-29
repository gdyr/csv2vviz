use std::{path::PathBuf, io::{Read, Write}, str::FromStr};
use csv::StringRecord;
use serde::Serialize;
use regex::Regex;

use euclid::{Rotation3D, Point3D, Angle, UnknownUnit, Translation3D};

use clap::Parser;

#[derive(Debug, Serialize)]
struct AgentTraversal {
    dx: f32,
    dy: f32,
    dz: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    dt: Option<f32>
}

#[derive(Debug, Serialize)]
struct AgentTraversals(Vec<AgentTraversal>);

impl From<Vec<csv::StringRecord>> for AgentTraversals {
    fn from(records: Vec<csv::StringRecord>) -> Self {
        let mut traversals: Vec<AgentTraversal> = vec![];
        // traversals.push(
        //     AgentTraversal {
        //         dt: None,
        //         dx: records[0][1].parse::<f32>().unwrap(),
        //         dy: records[0][3].parse::<f32>().unwrap(),
        //         dz: records[0][2].parse::<f32>().unwrap()
        //     }
        // );
        for (cur, prev) in records.iter().skip(1).zip(records.iter()) {
            traversals.push(
                AgentTraversal {
                    dt: Some((cur[0].parse::<f32>().unwrap() - prev[0].parse::<f32>().unwrap()) / 1000.0),
                    dx: cur[1].parse::<f32>().unwrap() - prev[1].parse::<f32>().unwrap(),
                    dy: cur[3].parse::<f32>().unwrap() - prev[3].parse::<f32>().unwrap(),
                    dz: cur[2].parse::<f32>().unwrap() - prev[2].parse::<f32>().unwrap()
                }
            );
        }
        AgentTraversals(traversals)
    }
}

#[derive(Debug, Serialize)]
struct ColorAction {
    r: u8,
    g: u8,
    b: u8,
    frames: Option<u32>
}

#[derive(Debug, Serialize)]
struct AgentDescription {
    #[serde(rename = "homeX")]
    home_x: f32,
    #[serde(rename = "homeY")]
    home_y: f32,
    #[serde(rename = "homeZ")]
    home_z: f32,
    #[serde(rename = "agentTraversal")]
    traversals: AgentTraversals
}

#[derive(Debug, Serialize)]
struct Payload {
    id: usize,
    #[serde(rename = "type")]
    payload_type: String,
    #[serde(rename = "payloadActions")]
    actions: Vec<ColorAction>
}

#[derive(Debug, Serialize)]
struct Performance {
    id: usize,
    #[serde(rename = "agentDescription")]
    description: AgentDescription,
    #[serde(rename = "payloadDescription")]
    payload: Vec<Payload>
}

#[derive(Debug, Serialize)]
struct Show {
    version: String,
    #[serde(rename = "defaultPositionRate")]
    default_position_rate: f32,
    #[serde(rename = "defaultColorRate")]
    default_color_rate: f32,
    performances: Vec<Performance>
}

fn csv2vviz(
    fname: PathBuf,
    rotation: Option<Rotation3D<f32, UnknownUnit, UnknownUnit>>,
    translation: Option<Translation3D<f32, UnknownUnit, UnknownUnit>>
) {

    let new_file = fname.with_extension("vviz");
    println!("Generating {}", new_file.to_str().unwrap());

    let zipfile = std::fs::File::open(fname)
        .expect("Failed to open zip archive.");
    
    let mut archive = zip::ZipArchive::new(zipfile)
        .expect("Failed to read zip archive.");

    let mut show = Show {
        version: "1.0".into(),
        default_position_rate: 4.0,
        default_color_rate: 4.0,
        performances: vec![]
    };

    let mut file_index = 0;
    while let Ok(mut file) = archive.by_index(file_index) {

        let mut csv_reader = csv::Reader::from_reader(file.by_ref());
        let raw_records: Vec<csv::StringRecord> = csv_reader.records().map(|x| x.unwrap()).collect();

        let records: Vec<StringRecord> = raw_records.iter().map(|record| {

            let mut point = Point3D::<f32, UnknownUnit>::new(
                record[1].parse::<f32>().unwrap(),
                record[3].parse::<f32>().unwrap(),
                record[2].parse::<f32>().unwrap()
            );

            if let Some(rotation) = rotation {
                point = rotation.transform_point3d(point);
            }

            if let Some(translation) = translation {
                point = translation.transform_point3d(&point);
            }

            let mut new_record = StringRecord::new();
            new_record.push_field(&record[0]);
            new_record.push_field(&point.x.to_string());
            new_record.push_field(&point.z.to_string());
            new_record.push_field(&point.y.to_string());
            new_record.push_field(&record[4]);
            new_record.push_field(&record[5]);
            new_record.push_field(&record[6]);
            new_record

        }).collect();

        let name_re = Regex::new(r"^Drone (\d+)").unwrap();

        let drone_id = name_re.captures(
            file.by_ref().name()
        ).unwrap().get(1).unwrap().as_str()
        .parse::<usize>().unwrap();

        show.performances.push(
            Performance {
                id: drone_id - 1, // vviz uses 0-indexing
                description: AgentDescription {
                    home_x: records[0][1].parse::<f32>().unwrap(),
                    home_y: records[0][3].parse::<f32>().unwrap(),
                    home_z: records[0][2].parse::<f32>().unwrap(),
                    traversals: records.into()
                },
                payload: vec![]
            }
        );

        show.performances.sort_by_cached_key(|p| p.id);
        
        file_index += 1;
    }

    let mut vviz_file = std::fs::File::create(new_file).expect("Failed to create new file.");
    vviz_file.write_all(
        serde_json::to_string(&show).expect("Failed to serialize show data.").as_bytes()
    ).expect("Failed to write new file.");

}

#[derive(Debug, Clone)]
struct F3D {
    x: f32,
    y: f32,
    z: f32
}

#[derive(Debug, PartialEq, Eq)]
struct ParseF3DError {
    error: String
}

#[derive(Debug, PartialEq, Eq)]
struct ParseFloatError {
    error: String
}

impl From<ParseFloatError> for ParseF3DError {
    fn from(_: ParseFloatError) -> Self {
        ParseF3DError {
            error: "Could not parse float".to_string(),
        }
    }
}

impl std::str::FromStr for F3D {
    type Err = ParseF3DError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let name_re = Regex::new(r"^([\d\.\-]+) ([\d\.\-]+) ([\d\.\-]+)$").unwrap();
        let matches = name_re.captures(s).ok_or_else(|| ParseF3DError { error: "invalid coordinate format".to_string() })?;
        Ok(F3D {
            x: matches.get(1).ok_or_else(|| ParseF3DError { error: "invalid first coordinate".to_string() })?.as_str().parse::<f32>().map_err(|_| ParseF3DError { error: "invalid first coordinate".to_string() })?,
            y: matches.get(2).ok_or_else(|| ParseF3DError { error: "invalid second coordinate".to_string() })?.as_str().parse::<f32>().map_err(|_| ParseF3DError { error: "invalid second coordinate".to_string() })?,
            z: matches.get(3).ok_or_else(|| ParseF3DError { error: "invalid third coordinate".to_string() })?.as_str().parse::<f32>().map_err(|_| ParseF3DError { error: "invalid third coordinate".to_string() })?
        })
    }
}

impl From<&str> for F3D {
    fn from(value: &str) -> Self {
        F3D::from_str(value).expect("Failed to parse point")
    }
}

#[derive(Parser, Debug)]
struct Args {

    filename: String,

    #[arg(short, long)]
    rotate: Option<F3D>,

    #[arg(short, long)]
    translate: Option<F3D>
}

fn main() {

    let args = Args::parse();

    println!("{:?}", args);

    let mut rotation: Option<Rotation3D<f32, UnknownUnit, UnknownUnit>> = None;
    if let Some(rot) = args.rotate {
        rotation = Some(Rotation3D::euler(
            Angle::degrees(rot.x),
            Angle::degrees(rot.y),
            Angle::degrees(rot.z)
        ).normalize());
    }

    let mut translation: Option<Translation3D<f32, UnknownUnit, UnknownUnit>> = None;
    if let Some(trans) = args.translate {
        translation = Some(Translation3D::new(
            trans.x,
            trans.y,
            trans.z
        ));
    }

    // let args: Vec<_> = std::env::args().collect();
    // if args.len() < 2 {
    //     println!("Usage: {} <filename.zip>", args[0]);
    //     return;
    // }

    let fname = PathBuf::from(args.filename);

    let extension = fname.extension().expect("Could not get file extension.");
    if extension == "zip" {
        csv2vviz(fname, rotation, translation);
    } else {
        panic!("Invalid file format.");
    }

}
