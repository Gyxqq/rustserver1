#[macro_use]
extern crate rocket;
use json;
use rocket::data::ToByteUnit;
use rocket::fs::FileServer;
use rocket::tokio::fs::{create_dir_all, File};
use rocket::tokio::io::AsyncWriteExt;
use rocket::Data;
use rocket::State;
use sqlite::Connection;
use std::collections::vec_deque::{self, VecDeque};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;
use std::{env, thread};
use std::sync::Arc;
use threadpool::ThreadPool;
use uuid::Uuid;
static db: Mutex<i32> = Mutex::new(0);
static config_passwd: &'static str = "123456";
fn img_process(filename: &String) {
    println!("img_process:{}", filename);
    thread::sleep(Duration::from_secs(5));
}
fn process(deque: &Arc<Mutex<VecDeque<String>>>, num: i32) {
    let pool = ThreadPool::new(num as usize);
    loop {
        let empty={
            let deque = deque.lock().unwrap();
            deque.is_empty()
        };

        if empty {
            sleep(Duration::from_secs(1));
            continue;
        }
        let filename = {
            let mut deque = deque.lock().unwrap();
            deque.pop_front().unwrap()
        };
        pool.execute(move || {
            img_process(&filename);
            println!("img_process:{}", &filename)
        });
    }
}


#[get("/hellow")]
fn hello() -> Result<String, std::io::Error> {
    Ok(format!("Hello!"))
}
#[get("/config/cleardatabase?<password>")]
fn cleardatabase(password:String) -> Result<String, std::io::Error> {
    if password !=config_passwd  {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "password error"));
    }
    let connect = db.lock().unwrap();
    let conn = Connection::open("img.db").unwrap();
    conn.execute("DELETE FROM img").unwrap();
    //create img table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS img (id INTEGER PRIMARY KEY,filename TEXT NOT NULL
        ,usertag TEXT NOT NULL,outline_score TEXT NOT NULL
        )",
    ).unwrap();
    Ok(format!("cleardatabase scuess!"))
}
#[post("/uploadimg?<type0>&<tag>", data = "<data>")]
async fn uploadimg(deque: &State<Arc<Mutex<VecDeque<String>>>>,type0: String,tag: String,data: Data<'_>) -> Result<String, std::io::Error> {
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
            filename, tag, "none"
        );
        conn.execute(query).unwrap();
    }
    {
        let mut deque = deque.lock().unwrap();
        deque.push_back(filename.clone());
        println!("deque add : {}", deque.back().unwrap());
    }
    let json = json::object! {
        "filename" => filename,
        "usertag" => tag
    };
    Ok(format!("{}", json.dump()))
}
#[get("/getimgstat_bytag?<tag>")]
fn getimgstat_bytag(tag: String) -> Result<String, std::io::Error> {
    let connect = db.lock().unwrap();
    let conn = Connection::open("img.db").unwrap();
    let query=format!("SELECT * FROM img WHERE usertag='{}'",tag);
    let mut stmt = conn.prepare(query).unwrap();
    let mut imglist = json::JsonValue::new_array();
    //为空返回异常
    if stmt.next().unwrap() == sqlite::State::Done {
        return Ok(format!("no img found by tag:{}",tag));
    }
    while let sqlite::State::Row = stmt.next().unwrap() {
        let filename: String = stmt.read(1).unwrap();
        let usertag: String = stmt.read(2).unwrap();
        let outline_score: String = stmt.read(3).unwrap();
        let img = json::object! {
            "filename" => filename,
            "usertag" => usertag,
            "outline_score" => outline_score
        };
        imglist.push(img).unwrap();
    }
    Ok(format!("{}", imglist.dump()))
}

#[launch]
fn rocket() -> _ {
    {
        env::set_var("ROCKET_LOG", "trace");
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
        let deque = Arc::new(Mutex::new(VecDeque::<String>::new()));
        let deque_clone = Arc::clone(&deque);
        thread::spawn(move || process(&deque_clone,4));
    
        rocket::build()
            .manage(deque)
            .mount("/", routes![hello])
            .mount("/", rocket::routes![uploadimg])
            .mount("/res", FileServer::from("res/"))
            .mount("/", routes![cleardatabase])
            .mount("/", routes![getimgstat_bytag])
}
