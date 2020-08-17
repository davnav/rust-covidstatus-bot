use std::env;
use std::fmt;
use chrono::{DateTime, TimeZone, NaiveDateTime,NaiveDate, Utc};
use std::collections::{HashMap,HashSet};

use futures::StreamExt;
use telegram_bot::*;
use telegram_bot::Api;
use redis::Commands;
use hyper::{Body, Method, Request, Uri};
use hyper::Client;
use hyper::body::HttpBody as _;
use tokio::io::{stdout, AsyncWriteExt as _};
use serde::{Serialize,Deserialize};
//use hyper_tls::HttpsConnector;

#[derive(Debug)]
pub enum MyError{
    tele_err(telegram_bot::Error),
    redis_err(redis::RedisError),
    hyper_err(hyper::Error),
    hyper_uri_err(hyper::http::uri::InvalidUri),
    serde_err(serde_json::Error),
    chrono_err(chrono::ParseError),
  //  std_err(std::option::NoneError),
}
//impl redis::RedisError for MyError{}
impl From<redis::RedisError> for MyError{
    fn from(error: redis::RedisError) -> Self{
        MyError::redis_err(error)
    }
}
impl From<telegram_bot::Error> for MyError{
    fn from(error: telegram_bot::Error) -> Self{
        MyError::tele_err(error)
    }
}
impl From<hyper::Error> for MyError{
    fn from(error: hyper::Error) -> Self{
        MyError::hyper_err(error)
    }
}
impl From<hyper::http::uri::InvalidUri> for MyError{
    fn from(error: hyper::http::uri::InvalidUri) -> Self{
        MyError::hyper_uri_err(error)
    }
}
impl From<serde_json::Error> for MyError{
    fn from(error: serde_json::Error) -> Self{
        MyError::serde_err(error)
    }
}
impl From<chrono::ParseError> for MyError{
    fn from(error: chrono::ParseError) -> Self{
        MyError::chrono_err(error)
    }
}
#[derive(Debug,Serialize,Deserialize,Default)]
struct CovidResponse{
    #[serde(rename="info")]  
    #[serde(flatten)]                  // <--- RIGHT HERE!
        info: HashMap<DateTime<Utc>, HashMap<String, Location>>,
        success:bool,

}

#[derive(Debug,Serialize,Deserialize,Default)]
struct Location {

    #[serde(rename(deserialize = "no_of_persons_discharged_from_home_isolation"))]
    discharged: Option<i32>,
    #[serde(rename(deserialize = "no_of_persons_hospitalized_today"))]
    hospitalized:Option<i32>,
    #[serde(rename(deserialize = "no_of_persons_under_home_isolation_as_on_today"))]
    home_isolation:Option<i32>,
    #[serde(rename(deserialize = "no_of_persons_under_observation_as_on_today"))]
    under_observation:Option<i32>,
    #[serde(rename(deserialize = "no_of_positive_cases_admitted"))]
    positive_cases:Option<i32>,
    #[serde(rename(deserialize = "no_of_symptomatic_persons_hospitalized_as_on_today"))]
    symptomatic:Option<i32>,
    other_districts:Option<HashMap<String,String>>, 
}

#[tokio::main]
async fn main() -> Result<(), MyError> {
    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");

    let districts:HashSet<_> = vec!("alappuzha","ernakulam","idukki","kannur",
                                "kasaragod","kollam","kottayam","kozhikode","malappuram","palakkad",
                                "pathanamthitta","thiruvananthapuram","thrissur","total",
                                "wayanad").iter().clone().map(ToString::to_string).collect();
    
    let api = Api::new(token);

    // Fetch new updates via long poll method
    let mut stream = api.stream();
    while let Some(update) = stream.next().await {
        let update = update?;
        if let UpdateKind::Message(message) = update.kind {
            if let MessageKind::Text { ref data, .. } = message.kind {
                println!("<{}>: {}", &message.from.first_name, data);
                let client = redis::Client::open("redis://127.0.0.1/")?;
                
                let mut con = client.get_connection()?;
               let _:() = con.set(&message.from.first_name.to_string(),data)?;

                let hyper_client = Client::new();
                println!("{}",&message.from.first_name.to_string());
                let loc = &data;

                
                if districts.get::<String>(&data.to_lowercase()).is_some(){
                            let covid = CovidResponse::default();
                            let uri = format!("http://covid19-kerala-api.herokuapp.com/api/location?loc={}&date=03-03-2020",loc.to_lowercase());
                            println!("{}",uri);
                            let uri = uri.parse::<Uri>()?;
                            let mut resp = hyper_client.get(uri).await?;
                            while let  Some(mut body) = resp.body_mut().data().await {
                                    match body{
                                    Ok(body) => { println!("{:?}",body);
                                                        let json:CovidResponse = serde_json::from_slice(&body)?;    
                                                        let date_str = "2020-03-03 00:00:00";
                                                        let date = Utc.datetime_from_str(&date_str, "%Y-%m-%d %H:%M:%S")?;
                                                   
                                                        println!("{:#?}",json.info.get(&date)); 
                                                        api.send(message.text_reply(format!(
                                                            "Hope that you are doing well! Covid cases in {},\n,
                                                            Number of persons under observation as on 03-03-2020 = {:#?},
                                                            Number of persons discharged from home isolation as on 03-03-2020 = {:#?}",
                                                            
                                                            data,
                                                                if let Some(loc1) =  json.info.get(&date){
                                                                    
                                                                        if let Some(value) = loc1.get(data){

                                                                            value.under_observation.unwrap()

                                                                        }else {
                                                                            0
                                                                        }
                                                                    
                                                                }else{
                                                                    0
                                                                },

                                                                if let Some(loc1) =  json.info.get(&date){
                                                                    
                                                                        if let Some(value) = loc1.get(data){

                                                                            value.discharged.unwrap()

                                                                        }else {
                                                                            0
                                                                        }
                                                                    
                                                                }else{
                                                                    0
                                                                }
                                                           
                                                             
                                                                
                                                                                                        
                                                        )))
                                                        .await?;
                                                    }
                                    Err(e) => { println!("{:?}",e);}
                                     } 
                            }
                        }else {
                            api.send(message.text_reply(format!(
                                "retry - place is not found in database!, {}!",
                                &message.from.first_name
                            )))
                            .await?;

                        };
                
            }
        }
    }
    Ok(())
}