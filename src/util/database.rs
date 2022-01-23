use std::{fmt::{Display, Debug}, env};
use sqlx::{Row, MySqlPool};

#[derive(Clone)]
pub struct DatabaseTool {
    pub pool: MySqlPool,
}

pub trait ValidValue: 'static + Display + Debug {}

impl ValidValue for u64 {}
impl ValidValue for u128 {}
impl ValidValue for &'static str {}

pub trait ValidInt: 'static + Display + Copy {
    fn as_i64(self) -> i64;
}

impl ValidInt for u64 {
    fn as_i64(self) -> i64 {
        self.try_into().expect("Value too large to be stored in database as an integer")
    }
}
impl ValidInt for i64 {
    fn as_i64(self) -> i64 {
        self
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ColumnType {
    which: &'static str,
}

pub const TEXT: ColumnType = ColumnType {which: "TEXT"};
pub const INTEGER: ColumnType = ColumnType {which: "BIGINT"};
pub const BOOL: ColumnType = ColumnType {which: "BOOLEAN"};

impl DatabaseTool {
    pub async fn retrieve_str<T: ValidInt>(&self, table: &str, wanted_column: &str, seeking_column: &str, id: &T) -> String {
        let row: (String,) = sqlx::query_as(format!("SELECT {} FROM {} WHERE {} = {}", wanted_column, table, seeking_column, id.as_i64()).as_str())
        .fetch_one(&self.pool)
        .await
        .expect("Could not query database [retrieve_str]");

        row.0
    }

    pub async fn retrieve_int<T: ValidInt>(&self, table: &str, wanted_column: &str, seeking_column: &str, seeking_id: &T) -> i64 {
        let row: (i64,) = sqlx::query_as(format!("SELECT {} FROM {} WHERE {} = {}", wanted_column, table, seeking_column, seeking_id.as_i64()).as_str())
        .fetch_one(&self.pool)
        .await
        .expect("Could not query database [retrieve_int]");

        row.0
    }

    pub async fn retrieve_bool<T: ValidInt>(&self, table: &str, wanted_column: &str, seeking_column: &str, seeking_id: &T) -> bool {
        let row: (bool,) = sqlx::query_as(format!("SELECT {} FROM {} WHERE {} = {}", wanted_column, table, seeking_column, seeking_id.as_i64()).as_str())
        .fetch_one(&self.pool)
        .await
        .expect("Could not query database [retrieve_int]");

        row.0
    }

    pub async fn update_str<T: ValidInt>(&self, table: &str, column: &str, value: &str, id: &T) {
        let value_scrubbed: String;
        if value.contains("'") {
            value_scrubbed = value.replace("'", "''");
        } else {
            value_scrubbed = String::from(value);
        }
        sqlx::query(format!("UPDATE {} SET {} = '{}' WHERE id = {}", table, column, value_scrubbed, id.as_i64()).as_str())
        .execute(&self.pool)
        .await
        .expect("Could not update database [update]");
    }

    pub async fn update_int<T: ValidInt, U: ValidInt>(&self, table: &str, column: &str, value: &T, id: &U) {
        sqlx::query(format!("UPDATE {} SET {} = {} WHERE id = {}", table, column, value.as_i64(), id.as_i64()).as_str())
        .execute(&self.pool)
        .await
        .expect("Could not update database [update]");
    }

    pub async fn update_bool<U: ValidInt>(&self, table: &str, column: &str, value: bool, id: &U) {
        let value_str: &str;
        if value {
            value_str = "1";
        } else {
            value_str = "0";
        }

        sqlx::query(format!("UPDATE {} SET {} = {} WHERE id = {}", table, column, value_str, id.as_i64()).as_str())
        .execute(&self.pool)
        .await
        .expect("Could not update database [update]");
    }

    /// Returns true if a row exists in a given table, with a given value, at a given column
    pub async fn row_exists<T: ValidValue>(&self, table: &str, column_name: &str, value: &T) -> bool{
        // let row: (bool,) = sqlx::query_as(format!("SELECT EXISTS(SELECT 1 FROM {} WHERE {} = '{}')", table, column_name, value).as_str())
        let row: (bool,) = sqlx::query_as(format!("SELECT EXISTS(SELECT {} FROM {} WHERE {} = '{}')", column_name, table, column_name, value).as_str())
        .fetch_one(&self.pool)
        .await
        .expect("Could not query database [row_exists]");

        row.0
    }

    // Should probably increase safety on this at some point
    pub async fn insert_row(&self, table: &str, values: &[&str]) {
        let mut qry = format!("INSERT INTO {} VALUES (", table);
        
        for s in values {
            match s.parse::<i64>() {
                Ok(i) => {
                    qry.push_str(&format!("{}, ", i));
                    continue;
                },
                _ => (),
            }

            match s.parse::<bool>() {
                Ok(b) => {
                    if b {
                        qry.push_str("1, ");
                    } else {
                        qry.push_str("0, ");
                    }
                    continue;
                }
                _ => (),
            }

            qry.push_str(&format!("'{}', ", s));
        }

        qry.pop();
        qry.pop();
        qry.push(')');

        sqlx::query(&qry)
        .execute(&self.pool)
        .await
        .expect("Could not insert into database [insert_row 3]");
    }

    pub async fn delete_row<T: ValidValue>(&self, table: &str, column_name: &str, value: &T) {
        sqlx::query(format!("DELETE FROM {} WHERE {} = {}", table, column_name, value).as_str())
        .execute(&self.pool)
        .await
        .expect("Could not update database [update]");
    }

    pub async fn table_exists(&self, table: &str) -> bool {
        let row: (bool,) = sqlx::query_as(format!("SELECT EXISTS(SELECT table_name FROM information_schema.tables WHERE table_schema = '{}' AND table_name = '{}')", env::var("MYSQL_DB").unwrap(), table).as_str())
        .fetch_one(&self.pool)
        .await
        .expect("Could not query database [table_exists]");

        row.0
    }

    // Should probably increase safety on this at some point
    pub async fn create_table(&self, table: &str, keys: &[&str], types: &[ColumnType]) {
        // Make sure there's a key for every declared type
        let keys_len = keys.len();
        if keys_len != types.len() {
            panic!("Mismatch between amount of keys and types provided\nkeys: {:?}\ntypes: {:?}", keys, types)
        }

        // Create the query for the database base on how many columns were provided
        let mut qry = format!("CREATE table IF NOT exists {}(", table);
        let last_index = keys_len - 1;
        if keys_len >= 2 {
            for (i, k) in keys[..last_index].into_iter().enumerate() {
                qry = format!("{}{} {}, ", qry, k, types[i].which)
            }
        }
        qry = format!("{}{} {})", qry, keys[last_index], types[last_index].which);

        sqlx::query(&qry)
        .execute(&self.pool)
        .await
        .expect("Could not create table [create_table]");
    }

    pub async fn get_all_rows(&self, table: &str, key: &str) -> Vec<u64> {
        let result = sqlx::query(format!("SELECT {} FROM {}", key, table).as_str())
        .fetch_all(&self.pool)
        .await
        .expect("Could not query database [get_all_rows]");

        result.iter()
            .map(|i| i.get::<i64, usize>(0) as u64)
            .collect()
    }
}