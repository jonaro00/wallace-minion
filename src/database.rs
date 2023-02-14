use std::{future::Future, sync::Arc};

use anyhow::{anyhow, Error, Result};
use async_trait::async_trait;

use crate::prisma::*;

#[async_trait]
pub trait WallaceDBClient {
    async fn do_trx<T, F, FFut>(&self, func: F) -> Result<T>
    where
        T: Send,
        FFut: Future<Output = Result<T>> + Send,
        F: FnOnce(Arc<PrismaClient>) -> FFut + Send;
    fn log_error(&self, err: impl std::error::Error, msg: &'static str) -> Error {
        println!("{err}");
        anyhow!(msg)
    }
    fn positive(&self, amount: i64) -> Result<()> {
        if !amount.is_positive() {
            return Err(anyhow!("Non-positive amount"));
        }
        Ok(())
    }
    async fn upsert_guild(&self, id: u64) -> Result<guild::Data>;
    async fn transfer_bank_account_balance(
        &self,
        from_user_id: u64,
        to_user_id: u64,
        amount: i64,
    ) -> Result<(i64, i64)>;
    async fn set_guild_default_name(&self, id: u64, value: String) -> Result<guild::Data>;
    async fn get_guild_default_name(&self, id: u64) -> Result<String>;
    async fn get_guild_random_names(&self, id: u64) -> Result<(Vec<String>, Vec<String>)>;
    async fn add_guild_random_name_subject(
        &self,
        id: u64,
        value: String,
    ) -> Result<rn_subject::Data>;
    async fn add_guild_random_name_object(&self, id: u64, value: String)
        -> Result<rn_object::Data>;
    async fn upsert_user(&self, id: u64) -> Result<user::Data>;
    async fn get_user_mature(&self, id: u64) -> Result<bool>;
    async fn set_user_mature(&self, id: u64, mature: bool) -> Result<user::Data>;
    async fn get_all_users(&self) -> Result<Vec<user::Data>>;
    async fn create_lol_account(
        &self,
        server: String,
        summoner: String,
        user_id: u64,
    ) -> Result<lol_account::Data>;
    async fn delete_lol_account(
        &self,
        server: String,
        summoner: String,
    ) -> Result<lol_account::Data>;
    async fn get_all_lol_accounts_in_user(&self, id: u64) -> Result<Vec<lol_account::Data>>;
    async fn get_bank_account(&self, user_id: u64) -> Result<bank_account::Data>;
    async fn create_bank_account(&self, user_id: u64) -> Result<bank_account::Data>;
    async fn delete_bank_account(&self, user_id: u64) -> Result<bank_account::Data>;
    async fn get_bank_account_balance(&self, user_id: u64) -> Result<i64>;
    async fn has_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<()>;
    async fn add_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<i64>;
    async fn subtract_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<i64>;
    async fn upsert_channel(&self, id: u64) -> Result<channel::Data>;
    async fn create_task(
        &self,
        cron: String,
        cmd: String,
        arg: Option<String>,
        channel_id: u64,
    ) -> Result<task::Data>;
    async fn delete_task(&self, id: i32) -> Result<task::Data>;
    async fn get_all_tasks(&self) -> Result<Vec<task::Data>>;
    async fn get_all_tasks_in_channel(&self, id: u64) -> Result<Vec<task::Data>>;
}

#[async_trait]
impl WallaceDBClient for PrismaClient {
    async fn do_trx<T, F, FFut>(&self, func: F) -> Result<T>
    where
        T: Send,
        FFut: Future<Output = Result<T>> + Send,
        F: FnOnce(Arc<PrismaClient>) -> FFut + Send,
    {
        let (tx, tx_client) = self._transaction().begin().await?;
        let a = Arc::new(tx_client);
        match func(a.clone()).await {
            ok @ Ok(_) => {
                let tx_client =
                    Arc::try_unwrap(a).expect("the reference in the transaction to be dropped");
                tx.commit(tx_client)
                    .await
                    .map_err(|_| anyhow!("Failed to commit transaction"))?;
                ok
            }
            err @ Err(_) => {
                let tx_client =
                    Arc::try_unwrap(a).expect("the reference in the transaction to be dropped");
                tx.rollback(tx_client)
                    .await
                    .map_err(|_| anyhow!("Failed to rollback transaction"))?;
                err
            }
        }
    }
    async fn upsert_guild(&self, id: u64) -> Result<guild::Data> {
        self.guild()
            .upsert(guild::id::equals(id as i64), (id as i64, vec![]), vec![])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to update guild"))
    }
    async fn set_guild_default_name(&self, id: u64, value: String) -> Result<guild::Data> {
        self.guild()
            .upsert(
                guild::id::equals(id as i64),
                (id as i64, vec![guild::default_name::set(Some(value.clone()))]),
                vec![guild::default_name::set(Some(value))],
            )
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to update guild"))
    }
    async fn get_guild_default_name(&self, id: u64) -> Result<String> {
        self.guild()
            .find_unique(guild::id::equals(id as i64))
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get guild"))?
            .ok_or_else(|| anyhow!("Guild not found"))?
            .default_name
            .ok_or_else(|| anyhow!("No default name set"))
    }
    async fn get_guild_random_names(&self, id: u64) -> Result<(Vec<String>, Vec<String>)> {
        let r = self
            .guild()
            .find_unique(guild::id::equals(id as i64))
            .with(guild::rn_subject::fetch(vec![]))
            .with(guild::rn_object::fetch(vec![]))
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get guild"))?
            .ok_or_else(|| anyhow!("Guild not found"))?;
        let subs = r
            .rn_subject()
            .map_err(|r| self.log_error(r, "Failed to fetch subjects"))?;
        let objs = r
            .rn_object()
            .map_err(|r| self.log_error(r, "Failed to fetch objects"))?;
        Ok((
            subs.iter().map(|s| s.value.clone()).collect(),
            objs.iter().map(|s| s.value.clone()).collect(),
        ))
    }
    async fn add_guild_random_name_subject(
        &self,
        id: u64,
        value: String,
    ) -> Result<rn_subject::Data> {
        self.upsert_guild(id).await?;
        self.rn_subject()
            .create(value, guild::id::equals(id as i64), vec![])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to create subject"))
    }
    async fn add_guild_random_name_object(
        &self,
        id: u64,
        value: String,
    ) -> Result<rn_object::Data> {
        self.upsert_guild(id).await?;
        self.rn_object()
            .create(value, guild::id::equals(id as i64), vec![])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to create object"))
    }
    async fn upsert_user(&self, id: u64) -> Result<user::Data> {
        self.user()
            .upsert(user::id::equals(id as i64), (id as i64, vec![]), vec![])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to update user"))
    }
    async fn get_user_mature(&self, id: u64) -> Result<bool> {
        self.user()
            .find_unique(user::id::equals(id as i64))
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get user"))
            .and_then(|u| u.ok_or_else(|| anyhow!("User not registered in Wallace")))
            .map(|u| u.mature)
    }
    async fn set_user_mature(&self, id: u64, mature: bool) -> Result<user::Data> {
        self.user()
            .update(user::id::equals(id as i64), vec![user::mature::set(mature)])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to update user"))
    }
    async fn get_all_users(&self) -> Result<Vec<user::Data>> {
        self.user()
            .find_many(vec![])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get users"))
    }
    async fn create_lol_account(
        &self,
        server: String,
        summoner: String,
        user_id: u64,
    ) -> Result<lol_account::Data> {
        self.upsert_user(user_id).await?;
        self.lol_account()
            .create(server, summoner, user::id::equals(user_id as i64), vec![])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to create LoL account"))
    }
    async fn delete_lol_account(
        &self,
        server: String,
        summoner: String,
    ) -> Result<lol_account::Data> {
        let a = self
            .lol_account()
            .find_first(vec![
                lol_account::server::equals(server),
                lol_account::summoner::equals(summoner),
            ])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get account"))?
            .ok_or_else(|| anyhow!("LoL account not found"))?;
        self.lol_account()
            .delete(lol_account::id::equals(a.id))
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to delete LoL account"))
    }
    async fn get_all_lol_accounts_in_user(&self, id: u64) -> Result<Vec<lol_account::Data>> {
        self.user()
            .find_unique(user::id::equals(id as i64))
            .with(user::lol_account::fetch(vec![]))
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get user"))?
            .ok_or_else(|| anyhow!("User not found"))?
            .lol_account()
            .cloned()
            .map_err(|r| self.log_error(r, "Failed to fetch LoL accounts"))
    }
    async fn get_bank_account(&self, user_id: u64) -> Result<bank_account::Data> {
        self.user()
            .find_unique(user::id::equals(user_id as i64))
            .with(user::bank_account::fetch())
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get user"))?
            .ok_or_else(|| anyhow!("User not found"))?
            .bank_account()
            .map_err(|r| self.log_error(r, "Failed to fetch bank account"))?
            .ok_or_else(|| anyhow!("Bank account not found"))
            .cloned()
    }
    async fn create_bank_account(&self, user_id: u64) -> Result<bank_account::Data> {
        if self.get_bank_account(user_id).await.is_ok() {
            return Err(anyhow!("Account already open"));
        }
        self.upsert_user(user_id).await?;
        self.bank_account()
            .create(vec![bank_account::user_id::set(user_id as i64)])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to create bank account"))
    }
    async fn delete_bank_account(&self, user_id: u64) -> Result<bank_account::Data> {
        self.bank_account()
            .delete(bank_account::user_id::equals(user_id as i64))
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to delete bank account"))
    }
    async fn get_bank_account_balance(&self, user_id: u64) -> Result<i64> {
        let a = self.get_bank_account(user_id).await?;
        Ok(a.balance)
    }
    async fn has_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<()> {
        self.positive(amount)?;
        if amount > self.get_bank_account_balance(user_id).await? {
            return Err(anyhow!("Account balance too low"));
        }
        Ok(())
    }
    async fn add_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<i64> {
        self.positive(amount)?;
        self.bank_account()
            .update(
                bank_account::user_id::equals(user_id as i64),
                vec![bank_account::balance::increment(amount)],
            )
            .exec()
            .await
            .map(|b| b.balance)
            .map_err(|q| self.log_error(q, "Failed to update balance"))
    }
    async fn subtract_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<i64> {
        self.positive(amount)?;
        self.has_bank_account_balance(user_id, amount).await?;
        self.bank_account()
            .update(
                bank_account::user_id::equals(user_id as i64),
                vec![bank_account::balance::decrement(amount)],
            )
            .exec()
            .await
            .map(|b| b.balance)
            .map_err(|q| self.log_error(q, "Failed to update balance"))
    }
    async fn transfer_bank_account_balance(
        &self,
        from_user_id: u64,
        to_user_id: u64,
        amount: i64,
    ) -> Result<(i64, i64)> {
        if from_user_id == to_user_id {
            return Err(anyhow!("Can't transfer to self"));
        }
        let (r1, r2) = self
            .do_trx(|tx_client| async move {
                let r1 = tx_client
                    .subtract_bank_account_balance(from_user_id, amount)
                    .await?;
                let r2 = tx_client
                    .add_bank_account_balance(to_user_id, amount)
                    .await?;
                Ok((r1, r2))
            })
            .await?;
        Ok((r1, r2))
    }
    async fn upsert_channel(&self, id: u64) -> Result<channel::Data> {
        self.channel()
            .upsert(channel::id::equals(id as i64), (id as i64, vec![]), vec![])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to update channel"))
    }
    async fn create_task(
        &self,
        cron: String,
        cmd: String,
        arg: Option<String>,
        channel_id: u64,
    ) -> Result<task::Data> {
        self.upsert_channel(channel_id).await?;
        self.task()
            .create(
                cron,
                cmd,
                channel::id::equals(channel_id as i64),
                vec![task::arg::set(arg)],
            )
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to create task"))
    }
    async fn delete_task(&self, id: i32) -> Result<task::Data> {
        self.task()
            .delete(task::id::equals(id))
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to delete task"))
    }
    async fn get_all_tasks(&self) -> Result<Vec<task::Data>> {
        self.task()
            .find_many(vec![])
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get tasks"))
    }
    async fn get_all_tasks_in_channel(&self, id: u64) -> Result<Vec<task::Data>> {
        self.channel()
            .find_unique(channel::id::equals(id as i64))
            .with(channel::task::fetch(vec![]))
            .exec()
            .await
            .map_err(|q| self.log_error(q, "Failed to get channel"))?
            .ok_or_else(|| anyhow!("Channel not found"))?
            .task()
            .cloned()
            .map_err(|r| self.log_error(r, "Failed to fetch tasks"))
    }
}
