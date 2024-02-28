#[macro_use]
extern crate rocket;

use rocket::data::ToByteUnit;
use rocket::fs::FileServer;
use rocket::tokio::fs::{create_dir_all, File};
use rocket::tokio::io::AsyncWriteExt;
use rocket::Data;
use sqlite::Connection;
use std::sync::Mutex;
use uuid::Uuid;
use std::env;
static db: Mutex<i32> = Mutex::new(0);

#[get("/hellow")]
fn hello() -> Result<String, std::io::Error> {
    Ok(format!("Hello!"))
}

#[post("/uploadimg?<type0>&<tag>", data = "<data>")]
async fn uploadimg(type0:String,tag:String,data: Data<'_>,) -> Result<String, std::io::Error> {
    let mut filename = Uuid::new_v4().to_string() + "." + &type0;
    filename = "img/".to_string() + &filename;
    create_dir_all("img/").await?;
    let mut file = File::create(&filename).await?;
    let stream = data.open(10.mebibytes()).into_bytes().await?;

    let buff = stream.into_inner();
    //把data作为imgdata结构体打开

    // for i in buff {
    //     file.write(&[i]).await?;
    // } 垃圾写法
    file.write_all(&buff).await?;
    file.flush().await.unwrap();
    drop(file);

    {
        let connection = db.lock().unwrap();
        let conn = Connection::open("img.db").unwrap();
        let query = format!(
            "INSERT INTO img (filename,usertag,outline_score) VALUES ('{}','{}','{}')",
            filename, tag,"none"
        );
        conn.execute(query).unwrap();
    }

    Ok(format!("load image success, filename: {},usertag:{}", filename,tag))
}

#[launch]
fn rocket() -> _ {
    {   env::set_var("ROCKET_LOG", "trace");
        let mut connect = db.lock().unwrap();

        let conn = Connection::open("img.db").unwrap();
        //open img table or create img table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS img (id INTEGER PRIMARY KEY,filename TEXT NOT NULL
            ,usertag TEXT NOT NULL,outline_score TEXT NOT NULL
            )",
        )
        .unwrap();
        println!("database open success");
    }

    rocket::build()
        .mount("/", routes![hello])
        .mount("/", rocket::routes![uploadimg])
        .mount("/res", FileServer::from("res/"))
}
