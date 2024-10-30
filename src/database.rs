use anyhow::{anyhow, Error, Result};
use sqlx::{Acquire, PgConnection, PgPool};
use tracing::warn;

use crate::model::{LoLAccount, Task, User};

fn log_error(err: impl std::error::Error, msg: &'static str) -> Error {
    warn!("Database error: {err}");
    anyhow!(msg)
}
fn positive(amount: i64) -> Result<()> {
    if !amount.is_positive() {
        return Err(anyhow!("Non-positive amount"));
    }
    Ok(())
}

pub trait WallaceDBClient {
    async fn upsert_guild(self, id: u64) -> Result<()>;
    async fn set_guild_default_name(self, id: u64, value: String) -> Result<()>;
    async fn get_guild_default_name(self, id: u64) -> Result<String>;
    async fn get_guild_random_names(self, id: u64) -> Result<(Vec<String>, Vec<String>)>;
    async fn add_guild_random_name_subject(self, id: u64, value: String) -> Result<()>;
    async fn add_guild_random_name_object(self, id: u64, value: String) -> Result<()>;
    async fn upsert_user(self, id: u64) -> Result<()>;
    async fn get_user_mature(self, id: u64) -> Result<bool>;
    async fn set_user_mature(self, id: u64, mature: bool) -> Result<()>;
    async fn get_all_users(self) -> Result<Vec<User>>;
    async fn create_lol_account(
        self,
        server: String,
        name: String,
        tag: String,
        user_id: u64,
    ) -> Result<()>;
    async fn delete_lol_account(self, server: String, name: String, tag: String) -> Result<()>;
    async fn get_all_lol_accounts_in_user(self, id: u64) -> Result<Vec<LoLAccount>>;
    async fn create_bank_account(self, user_id: u64) -> Result<()>;
    async fn delete_bank_account(self, user_id: u64) -> Result<()>;
    async fn get_bank_account_balance(self, user_id: u64) -> Result<i64>;
    async fn has_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()>;
    async fn add_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()>;
    async fn subtract_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()>;
    async fn transfer_bank_account_balance(
        self,
        from_user_id: u64,
        to_user_id: u64,
        amount: i64,
    ) -> Result<()>;
    async fn upsert_channel(self, id: u64) -> Result<()>;
    async fn create_task(
        self,
        cron: String,
        cmd: String,
        arg: Option<String>,
        channel_id: u64,
    ) -> Result<()>;
    async fn delete_task(self, id: i32) -> Result<()>;
    async fn get_all_tasks(self) -> Result<Vec<Task>>;
    async fn get_all_tasks_in_channel(self, id: u64) -> Result<Vec<Task>>;
}

impl WallaceDBClient for &mut PgConnection {
    async fn upsert_guild(self, id: u64) -> Result<()> {
        sqlx::query("INSERT INTO guild (id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to upsert guild"))
    }
    async fn set_guild_default_name(self, id: u64, value: String) -> Result<()> {
        sqlx::query("UPDATE guild SET default_name = $1 WHERE id = $2")
            .bind(value)
            .bind(id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to update guild"))
    }
    async fn get_guild_default_name(self, id: u64) -> Result<String> {
        sqlx::query_as::<_, (Option<String>,)>("SELECT default_name FROM guild WHERE id = $1")
            .bind(id as i64)
            .fetch_one(self)
            .await?
            .0
            .ok_or_else(|| anyhow!("No default name"))
    }
    async fn get_guild_random_names(self, id: u64) -> Result<(Vec<String>, Vec<String>)> {
        let subs = sqlx::query_as::<_, (String,)>(
            "SELECT r.value FROM guild g JOIN rn_subject r ON g.id = r.guild_id WHERE id = $1",
        )
        .bind(id as i64)
        .fetch_all(self.as_mut())
        .await
        .map_err(|r| log_error(r, "Failed to fetch subjects"))?
        .into_iter()
        .map(|s| s.0)
        .collect();
        let objs = sqlx::query_as::<_, (String,)>(
            "SELECT r.value FROM guild g JOIN rn_object r ON g.id = r.guild_id WHERE id = $1",
        )
        .bind(id as i64)
        .fetch_all(self)
        .await
        .map_err(|r| log_error(r, "Failed to fetch objects"))?
        .into_iter()
        .map(|s| s.0)
        .collect();
        Ok((subs, objs))
    }
    async fn add_guild_random_name_subject(self, id: u64, value: String) -> Result<()> {
        self.upsert_guild(id).await?;
        sqlx::query("INSERT INTO rn_subject VALUES ($1, $2)")
            .bind(id as i64)
            .bind(value)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to create subject"))
    }
    async fn add_guild_random_name_object(self, id: u64, value: String) -> Result<()> {
        self.upsert_guild(id).await?;
        sqlx::query("INSERT INTO rn_object VALUES ($1, $2)")
            .bind(id as i64)
            .bind(value)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to create object"))
    }
    async fn upsert_user(self, id: u64) -> Result<()> {
        sqlx::query(r#"INSERT INTO "user" (id) VALUES ($1) ON CONFLICT DO NOTHING"#)
            .bind(id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to upsert user"))
    }
    async fn get_user_mature(self, id: u64) -> Result<bool> {
        sqlx::query_as::<_, (bool,)>(r#"SELECT mature FROM "user" WHERE id = $1"#)
            .bind(id as i64)
            .fetch_one(self)
            .await
            .map(|u| u.0)
            .map_err(|q| log_error(q, "Failed to get user"))
    }
    async fn set_user_mature(self, id: u64, mature: bool) -> Result<()> {
        sqlx::query(r#"UPDATE "user" SET mature = $1 WHERE id = $2"#)
            .bind(mature)
            .bind(id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to update user"))
    }
    async fn get_all_users(self) -> Result<Vec<User>> {
        sqlx::query_as(r#"SELECT * FROM "user""#)
            .fetch_all(self)
            .await
            .map_err(|q| log_error(q, "Failed to get users"))
    }
    async fn create_lol_account(
        self,
        server: String,
        name: String,
        tag: String,
        user_id: u64,
    ) -> Result<()> {
        self.upsert_user(user_id).await?;
        sqlx::query("INSERT INTO lol_account (server, name, tag, user_id) VALUES ($1, $2, $3, $4)")
            .bind(server)
            .bind(name)
            .bind(tag)
            .bind(user_id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to create LoL account"))
    }
    async fn delete_lol_account(self, server: String, name: String, tag: String) -> Result<()> {
        sqlx::query("DELETE FROM lol_account WHERE server = $1 AND name = $2 AND tag = $3")
            .bind(server)
            .bind(name)
            .bind(tag)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to delete LoL account"))
    }
    async fn get_all_lol_accounts_in_user(self, id: u64) -> Result<Vec<LoLAccount>> {
        sqlx::query_as::<_, LoLAccount>(r#"SELECT l.server, l.name, l.tag FROM "user" u JOIN lol_account l ON u.id = l.user_id WHERE u.id = $1"#)
            .bind(id as i64)
            .fetch_all(self)
            .await
            .map_err(|q| log_error(q, "Failed to get LoL accounts"))
    }
    async fn create_bank_account(self, user_id: u64) -> Result<()> {
        self.upsert_user(user_id).await?;
        sqlx::query("INSERT INTO bank_account (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(user_id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to create bank account"))
    }
    async fn delete_bank_account(self, user_id: u64) -> Result<()> {
        sqlx::query("DELETE FROM bank_account WHERE user_id = $1")
            .bind(user_id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to delete bank account"))
    }
    async fn get_bank_account_balance(self, user_id: u64) -> Result<i64> {
        sqlx::query_as::<_, (i64,)>("SELECT balance FROM bank_account WHERE user_id = $1")
            .bind(user_id as i64)
            .fetch_one(self)
            .await
            .map(|u| u.0)
            .map_err(|q| log_error(q, "Failed to get balance"))
    }
    async fn has_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()> {
        positive(amount)?;
        if amount > self.get_bank_account_balance(user_id).await? {
            return Err(anyhow!("Account balance too low"));
        }
        Ok(())
    }
    async fn add_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()> {
        positive(amount)?;
        sqlx::query("UPDATE bank_account SET balance = balance + $1 WHERE user_id = $2")
            .bind(amount)
            .bind(user_id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to update balance"))
    }
    async fn subtract_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()> {
        positive(amount)?;
        let mut tr = self.begin().await?;
        (&mut tr).has_bank_account_balance(user_id, amount).await?;
        sqlx::query("UPDATE bank_account SET balance = balance - $1 WHERE user_id = $2")
            .bind(amount)
            .bind(user_id as i64)
            .execute(&mut *tr)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to update balance"))?;
        tr.commit().await?;
        Ok(())
    }
    async fn transfer_bank_account_balance(
        self,
        from_user_id: u64,
        to_user_id: u64,
        amount: i64,
    ) -> Result<()> {
        if from_user_id == to_user_id {
            return Err(anyhow!("Can't transfer to self"));
        }
        let mut tr = self.begin().await?;
        (&mut tr)
            .subtract_bank_account_balance(from_user_id, amount)
            .await?;
        (&mut tr)
            .add_bank_account_balance(to_user_id, amount)
            .await?;
        tr.commit().await?;
        Ok(())
    }
    async fn upsert_channel(self, id: u64) -> Result<()> {
        sqlx::query("INSERT INTO channel VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to upsert channel"))
    }
    async fn create_task(
        self,
        cron: String,
        cmd: String,
        arg: Option<String>,
        channel_id: u64,
    ) -> Result<()> {
        self.upsert_channel(channel_id).await?;
        sqlx::query("INSERT INTO task (cron, cmd, arg, channel_id) VALUES ($1, $2, $3, $4)")
            .bind(cron)
            .bind(cmd)
            .bind(arg)
            .bind(channel_id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to create task"))
    }
    async fn delete_task(self, id: i32) -> Result<()> {
        sqlx::query("DELETE FROM task WHERE id = $1")
            .bind(id as i64)
            .execute(self)
            .await
            .map(|_| ())
            .map_err(|q| log_error(q, "Failed to delete task"))
    }
    async fn get_all_tasks(self) -> Result<Vec<Task>> {
        sqlx::query_as("SELECT * FROM task")
            .fetch_all(self)
            .await
            .map_err(|q| log_error(q, "Failed to get tasks"))
    }
    async fn get_all_tasks_in_channel(self, id: u64) -> Result<Vec<Task>> {
        sqlx::query_as("SELECT * FROM task WHERE channel_id = $1")
            .bind(id as i64)
            .fetch_all(self)
            .await
            .map_err(|q| log_error(q, "Failed to get tasks"))
    }
}

impl WallaceDBClient for &PgPool {
    async fn upsert_guild(self, id: u64) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.upsert_guild(id).await
    }
    async fn set_guild_default_name(self, id: u64, value: String) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.set_guild_default_name(id, value).await
    }
    async fn get_guild_default_name(self, id: u64) -> Result<String> {
        let mut conn = self.acquire().await?;
        conn.get_guild_default_name(id).await
    }
    async fn get_guild_random_names(self, id: u64) -> Result<(Vec<String>, Vec<String>)> {
        let mut conn = self.acquire().await?;
        conn.get_guild_random_names(id).await
    }
    async fn add_guild_random_name_subject(self, id: u64, value: String) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.add_guild_random_name_subject(id, value).await
    }
    async fn add_guild_random_name_object(self, id: u64, value: String) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.add_guild_random_name_object(id, value).await
    }
    async fn upsert_user(self, id: u64) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.upsert_user(id).await
    }
    async fn get_user_mature(self, id: u64) -> Result<bool> {
        let mut conn = self.acquire().await?;
        conn.get_user_mature(id).await
    }
    async fn set_user_mature(self, id: u64, mature: bool) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.set_user_mature(id, mature).await
    }
    async fn get_all_users(self) -> Result<Vec<User>> {
        let mut conn = self.acquire().await?;
        conn.get_all_users().await
    }
    async fn create_lol_account(
        self,
        server: String,
        name: String,
        tag: String,
        user_id: u64,
    ) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.create_lol_account(server, name, tag, user_id).await
    }
    async fn delete_lol_account(self, server: String, name: String, tag: String) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.delete_lol_account(server, name, tag).await
    }
    async fn get_all_lol_accounts_in_user(self, id: u64) -> Result<Vec<LoLAccount>> {
        let mut conn = self.acquire().await?;
        conn.get_all_lol_accounts_in_user(id).await
    }
    async fn create_bank_account(self, user_id: u64) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.create_bank_account(user_id).await
    }
    async fn delete_bank_account(self, user_id: u64) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.delete_bank_account(user_id).await
    }
    async fn get_bank_account_balance(self, user_id: u64) -> Result<i64> {
        let mut conn = self.acquire().await?;
        conn.get_bank_account_balance(user_id).await
    }
    async fn has_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.has_bank_account_balance(user_id, amount).await
    }
    async fn add_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.add_bank_account_balance(user_id, amount).await
    }
    async fn subtract_bank_account_balance(self, user_id: u64, amount: i64) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.subtract_bank_account_balance(user_id, amount).await
    }
    async fn transfer_bank_account_balance(
        self,
        from_user_id: u64,
        to_user_id: u64,
        amount: i64,
    ) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.transfer_bank_account_balance(from_user_id, to_user_id, amount)
            .await
    }
    async fn upsert_channel(self, id: u64) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.upsert_channel(id).await
    }
    async fn create_task(
        self,
        cron: String,
        cmd: String,
        arg: Option<String>,
        channel_id: u64,
    ) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.create_task(cron, cmd, arg, channel_id).await
    }
    async fn delete_task(self, id: i32) -> Result<()> {
        let mut conn = self.acquire().await?;
        conn.delete_task(id).await
    }
    async fn get_all_tasks(self) -> Result<Vec<Task>> {
        let mut conn = self.acquire().await?;
        conn.get_all_tasks().await
    }
    async fn get_all_tasks_in_channel(self, id: u64) -> Result<Vec<Task>> {
        let mut conn = self.acquire().await?;
        conn.get_all_tasks_in_channel(id).await
    }
}
