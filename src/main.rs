#[macro_use]
extern crate rocket;
use json;
// use rand::{self, Rng};
use rocket::data::ToByteUnit;
use rocket::fs::relative;
use rocket::fs::FileServer;
use rocket::fs::NamedFile;
use rocket::tokio::fs::{create_dir_all, File};
use rocket::tokio::io::AsyncWriteExt;
use rocket::Data;
use rocket::State;
use sqlite::Connection;
use std::collections::vec_deque::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Termination};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;
use std::{env, thread};
use threadpool::ThreadPool;
use uuid::Uuid;
static config_passwd: &'static str = "123456";
fn img_process(filename: &String, db: Arc<Mutex<Connection>>) {
    println!("img_process start :{}", filename);
    // let mut rng = rand::thread_rng();
    // let n: u64 = rng.gen_range(1..100);
    // thread::sleep(Duration::from_micros(n * 100));
    let output = Command::new("python")
        .arg("outlier_detect.py")
        .arg(filename)
        .output()
        .expect("failed to execute process");

    {
        let conn = db.lock().unwrap();
        let query = format!(
            "UPDATE img \n SET outline_score='{}' WHERE filename='{}'",
            String::from_utf8_lossy(&output.stdout),
            filename
        );
        conn.execute(query).unwrap();
        let err = String::from_utf8_lossy(&output.stderr);
        if err != "" {
            std::fs::create_dir_all("errlog/").unwrap();
            let name = "errlog/".to_string() + &uuid::Uuid::new_v4().to_string() + ".txt";
            let mut file = std::fs::File::create(&name).unwrap();
            file.write_all(err.as_bytes()).unwrap();
            let query = format!(
                "UPDATE img SET  err = '{}' WHERE filename='{}'",
                &name, filename
            );
            conn.execute(query).unwrap();
        }
        println!("err:{}", err);
    }
}
fn process(deque: &Arc<Mutex<VecDeque<String>>>, num: i32, db: Arc<Mutex<Connection>>) {
    let pool = ThreadPool::new(num as usize);
    loop {
        let empty = {
            let deque = deque.lock().unwrap();
            deque.is_empty()
        };

        if empty {
            sleep(Duration::from_secs(5));
            continue;
        }

        let filename = {
            let mut deque = deque.lock().unwrap();
            println!("now deque len:{}", deque.len());
            deque.pop_front().unwrap()
        };
        let db0 = Arc::clone(&db);
        pool.execute(move || {
            img_process(&filename, db0);
            println!("img_process over:{}", &filename)
        });
    }
}

#[get("/hellow")]
fn hello() -> Result<String, std::io::Error> {
    Ok(format!("Hello!"))
}
#[get("/")]
async fn index() -> Option<NamedFile> {
    NamedFile::open(Path::new("assets/").join("index.html"))
        .await
        .ok()
}

#[get("/config/cleardatabase?<password>")]
async fn cleardatabase(
    password: String,
    db: &State<Arc<Mutex<Connection>>>,
) -> Result<String, std::io::Error> {
    if password != config_passwd {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "password error",
        ));
    }

    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM img").unwrap();
    //create img table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS img (id INTEGER PRIMARY KEY,filename TEXT NOT NULL
        ,usertag TEXT NOT NULL,outline_score TEXT NOT NULL err TEXT
        )",
    )
    .unwrap();
    Ok(format!("cleardatabase scuess!"))
}
#[post("/uploadimg?<type0>&<tag>", data = "<data>")]
async fn uploadimg(
    deque: &State<Arc<Mutex<VecDeque<String>>>>,
    type0: String,
    tag: String,
    data: Data<'_>,
    db: &State<Arc<Mutex<Connection>>>,
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
        let conn = db.lock().unwrap();
        let query = format!(
            "INSERT INTO img (filename,usertag,outline_score,err) VALUES ('{}','{}','{}','{}')",
            filename, tag, "none", "none"
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
async fn getimgstat_bytag(
    tag: String,
    db: &State<Arc<Mutex<Connection>>>,
) -> Result<String, std::io::Error> {
    let conn = db.lock().unwrap();
    let query = format!("SELECT * FROM img WHERE usertag='{}'", tag);
    let mut stmt = conn.prepare(query).unwrap();
    let mut imglist = json::JsonValue::new_array();
    //为空返回异常
    if stmt.next().unwrap() == sqlite::State::Done {
        return Ok(format!("no img found by tag:{}", tag));
    }
    while let sqlite::State::Row = stmt.next().unwrap() {
        let filename: String = stmt.read(1).unwrap();
        let usertag: String = stmt.read(2).unwrap();
        let outline_score: String = stmt.read(3).unwrap();
        let err: String = stmt.read(4).unwrap();
        let img = json::object! {
            "filename" => filename,
            "usertag" => usertag,
            "outline_score" => outline_score,
            "err" => err
        };
        imglist.push(img).unwrap();
    }
    Ok(format!("{}", imglist.dump()))
}
#[get("/getimgstat_byfilename?<filename>")]
async fn getimgstat_byfilename(
    filename: String,
    db: &State<Arc<Mutex<Connection>>>,
) -> Result<String, std::io::Error> {
    let conn = db.lock().unwrap();
    let query = format!("SELECT * FROM img WHERE filename='{}'", filename);
    let mut stmt = conn.prepare(query).unwrap();

    //为空返回异常
    if stmt.next().unwrap() == sqlite::State::Done {
        return Ok(format!("no img found by filename:{}", filename));
    }
    let filename: String = stmt.read(1).unwrap();
    let usertag: String = stmt.read(2).unwrap();
    let outline_score: String = stmt.read(3).unwrap();
    let err: String = stmt.read(4).unwrap();
    let img = json::object! {
        "filename" => filename,
        "usertag" => usertag,
        "outline_score" => outline_score,
        "err" => err

    };
    Ok(format!("{}", img.dump()))
}
#[get("/getdequelen")]
async fn getdequelen(db: &State<Arc<Mutex<Connection>>>) -> Result<String, std::io::Error> {
    let conn = db.lock().unwrap();
    //seclect count  outline_score=none
    let query = format!("SELECT count(*) FROM img WHERE outline_score='none'");
    let mut stmt = conn.prepare(query).unwrap();
    stmt.next().unwrap();
    let count: i64 = stmt.read(0).unwrap();
    //json
    let json = json::object! {
        "dequelen" => count
    };
    Ok(format!("{}", json.dump()))
    // let count = stmt.next().unwrap();
}
#[launch]
fn rocket() -> _ {
    env::set_var("ROCKET_LOG", "trace");
    // let mut connect = db.lock().unwrap();
    // let
    let conn = Connection::open("img.db").unwrap();
    //open img table or create img table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS img (id INTEGER PRIMARY KEY,filename TEXT NOT NULL
            ,usertag TEXT NOT NULL,outline_score TEXT ,err TEXT
            )",
    )
    .unwrap();
    println!("database open success");
    let conn = Arc::new(Mutex::new(Connection::open("img.db").unwrap()));
    let deque = Arc::new(Mutex::new(VecDeque::<String>::new()));
    let deque_clone = Arc::clone(&deque);
    let conn_clone = Arc::clone(&conn);
    thread::spawn(move || process(&deque_clone, 4, conn_clone));

    rocket::build()
        .manage(deque)
        .manage(conn)
        .mount("/", routes![hello])
        .mount("/", rocket::routes![uploadimg])
        .mount("/", routes![cleardatabase])
        .mount("/", routes![getimgstat_bytag])
        .mount("/", routes![getimgstat_byfilename])
        .mount("/", routes![getdequelen])
        .mount("/", routes![index])
        .mount("/img", FileServer::from(".//img"))
        .mount("/assets", FileServer::from("./assets"))
}
