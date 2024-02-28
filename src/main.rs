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
use uuid::Uuid;
static db: Mutex<i32> = Mutex::new(0);
//static deque : Mutex<i32> = Mutex::new(0);
fn process(deque: &Arc<Mutex<VecDeque<String>>>, len: i32) {
    loop {
        let mut now_queue = VecDeque::<String>::new();
        {
            let mut deque = deque.lock().unwrap();
            now_queue = deque.clone();
        }
        if now_queue.is_empty() {
            sleep(Duration::from_secs(5));
            continue;
        }
        if now_queue.len() < len as usize {
            for i in 1..now_queue.len() {
                println!("deque pop : {}", now_queue.pop_front().unwrap());
                thread::spawn(|| {
                    sleep(Duration::from_secs(5));
                })
                .join()
                .unwrap();
            }
            for i in 1..now_queue.len() {
                let mut deque = deque.lock().unwrap();
                deque.pop_front();
            }
        } else {
            for i in 1..len {
                println!("deque pop : {}", now_queue.pop_front().unwrap());
                thread::spawn(|| {
                    sleep(Duration::from_secs(5));
                })
                .join()
                .unwrap();
            }
            for i in 1..len {
                let mut deque = deque.lock().unwrap();
                deque.pop_front();
            }
        }
    }
}
#[get("/hellow")]
fn hello() -> Result<String, std::io::Error> {
    Ok(format!("Hello!"))
}

#[post("/uploadimg?<type0>&<tag>", data = "<data>")]
async fn uploadimg(
    deque: &State<Arc<Mutex<VecDeque<String>>>>,
    type0: String,
    tag: String,
    data: Data<'_>,
) -> Result<String, std::io::Error> {
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
}
