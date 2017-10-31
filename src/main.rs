#[macro_use] extern crate clap;
extern crate osmpbfreader;
extern crate postgis;
#[macro_use] extern crate postgres;
#[macro_use] extern crate postgres_derive;

use osmpbfreader::OsmPbfReader;
use osmpbfreader::objects::{OsmId, OsmObj};
use postgis::ewkb::AsEwkbPoint;
use postgis::twkb::Point;
use postgres::{Connection, TlsMode};
use std::fs::File;

#[derive(Debug, ToSql, FromSql)]
#[postgres(name = "point_type")]
enum Type {
    #[postgres(name = "amenity")]
    Amenity,
    #[postgres(name = "shop")]
    Shop,
}

fn main() {
    let matches = clap_app!(myapp =>
        (about: "Import points from OSM")
        (@arg db: -d --("database-url") +takes_value +required env("DATABASE_URL") "Sets a database URL")
        (@arg input: +required "Sets the input file to use")
    ).get_matches();

    let database_url = matches.value_of("db").unwrap();
    let conn = Connection::connect(database_url, TlsMode::None).unwrap();

    let add_point = conn.prepare(
        "INSERT INTO points (id, location, type, subtype, name, email, phone, website, opening_hours, operator) \
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
    ).unwrap();

    let add_tag = conn.prepare(
        "INSERT INTO tags (point_id, key, value) VALUES ($1, $2, $3)"
    ).unwrap();

    let file_path = matches.value_of("input").unwrap(); 
    let file = File::open(file_path).unwrap();

    let mut pbf = OsmPbfReader::new(&file);

    for obj in pbf.iter().map(Result::unwrap) {
        if !obj.is_node() {
            continue;
        }

        if !obj.tags().contains_key("amenity") && !obj.tags().contains_key("shop") {
            continue;
        }

        let id = get_object_id(&obj);
        let tags = obj.tags();
        let location = Point {
            x: obj.node().unwrap().lat(),
            y: obj.node().unwrap().lon(),
        };

        add_point.execute(&[
            &id, &location.as_ewkb(), &Type::Amenity, &tags.get("amenity"), &tags.get("name"), &tags.get("email"),
            &tags.get("phone"), &tags.get("website"), &tags.get("opening_hours"), &tags.get("operator")
        ]).unwrap();

        for tag in obj.tags().iter() {
            match tag.0.as_ref() {
                "amenity" | "shop" => continue,
                "created_by" | "name" | "source" => continue,
                "email" | "phone" | "website" => continue,
                "opening_hours" | "operator" => continue,
                _ => ()
            }

            add_tag.execute(&[&id, &tag.0, &tag.1]).unwrap();
        }
    }
}

fn get_object_id(obj: &OsmObj) -> i64 {
    match obj.id() {
        OsmId::Node(id) => id.0,
        OsmId::Way(id) => id.0,
        OsmId::Relation(id) => id.0,
    }
}
