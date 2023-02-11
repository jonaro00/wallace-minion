use anyhow::{anyhow, Result};
#[allow(unused_imports)]
use sea_orm::{
    entity::*, query::*, sea_query::OnConflict, DatabaseConnection, DatabaseTransaction,
};

use entity::{prelude::*, *};

pub struct DBHandler {
    pub db: DatabaseConnection,
}

impl DBHandler {
    pub async fn begin(&self) -> Result<DatabaseTransaction> {
        self.db
            .begin()
            .await
            .map_err(|_| anyhow!("Database call failed"))
    }
    pub async fn commit(&self, trx: DatabaseTransaction) -> Result<()> {
        trx.commit()
            .await
            .map_err(|_| anyhow!("Database call failed"))
    }
    fn positive(&self, amount: i64) -> Result<()> {
        if !amount.is_positive() {
            return Err(anyhow!("Non-positive amount"));
        }
        Ok(())
    }
    pub async fn create_guild(&self, id: u64) -> Result<guild::Model> {
        Guild::insert(guild::ActiveModel {
            id: Set(id as i64),
            default_name: Set(None),
        })
        .on_conflict(
            OnConflict::column(guild::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_with_returning(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to create guild"))
    }
    pub async fn set_guild_default_name(&self, id: u64, value: String) -> Result<guild::Model> {
        let _ = self.create_guild(id).await;
        Guild::update(guild::ActiveModel {
            id: Set(id as i64),
            default_name: Set(Some(value)),
        })
        .exec(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to update guild"))
    }
    pub async fn get_guild_random_names(&self, id: u64) -> Result<(Vec<String>, Vec<String>)> {
        let subs = Guild::find_by_id(id as i64)
            .find_with_related(RnSubject)
            .all(&self.db)
            .await
            .map_err(|_| anyhow!("Database call failed"))?
            .first()
            .ok_or(anyhow!("No subjects in this guild"))?
            .1
            .iter()
            .map(|m| m.value.clone())
            .collect::<Vec<_>>();
        let obs = Guild::find_by_id(id as i64)
            .find_with_related(RnObject)
            .all(&self.db)
            .await
            .map_err(|_| anyhow!("Database call failed"))?
            .first()
            .ok_or(anyhow!("No objects in this guild"))?
            .1
            .iter()
            .map(|m| m.value.clone())
            .collect::<Vec<_>>();
        Ok((subs, obs))
    }
    pub async fn add_guild_random_name_subject(
        &self,
        id: u64,
        value: String,
    ) -> Result<rn_subject::Model> {
        let _ = self.create_guild(id).await;
        RnSubject::insert(rn_subject::ActiveModel {
            guild_id: Set(id as i64),
            value: Set(value),
        })
        .exec_with_returning(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to update guild"))
    }
    pub async fn add_guild_random_name_object(
        &self,
        id: u64,
        value: String,
    ) -> Result<rn_object::Model> {
        let _ = self.create_guild(id).await;
        RnObject::insert(rn_object::ActiveModel {
            guild_id: Set(id as i64),
            value: Set(value),
        })
        .exec_with_returning(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to update guild"))
    }
    pub async fn create_user(&self, id: u64) -> Result<user::Model> {
        User::insert(user::ActiveModel {
            id: Set(id as i64),
            bank_account_id: Set(None),
            ..Default::default()
        })
        .on_conflict(OnConflict::column(user::Column::Id).do_nothing().to_owned())
        .exec_with_returning(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to create user"))
    }
    pub async fn get_user_mature(&self, id: u64) -> Result<bool> {
        Ok(User::find_by_id(id as i64)
            .one(&self.db)
            .await
            .map_err(|_| anyhow!("Database call failed"))?
            .ok_or(anyhow!("User not registered in Wallace"))?
            .mature)
    }
    pub async fn set_user_mature(&self, id: u64, mature: bool) -> Result<user::Model> {
        User::update(user::ActiveModel {
            id: Set(id as i64),
            mature: Set(mature),
            bank_account_id: NotSet,
        })
        .exec(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to update user"))
    }
    pub async fn get_all_users(&self) -> Result<Vec<user::Model>> {
        User::find()
            .all(&self.db)
            .await
            .map_err(|_| anyhow!("Failed to get users"))
    }
    pub async fn create_channel(&self, id: u64) -> Result<channel::Model> {
        Channel::insert(channel::ActiveModel { id: Set(id as i64) })
            .on_conflict(
                OnConflict::column(channel::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await
            .map_err(|_| anyhow!("Failed to create channel"))
    }
    pub async fn create_lol_account(
        &self,
        server: String,
        summoner: String,
        user_id: u64,
    ) -> Result<lol_account::Model> {
        let _ = self.create_user(user_id).await;
        LolAccount::insert(lol_account::ActiveModel {
            id: NotSet,
            server: Set(server),
            summoner: Set(summoner),
            user_id: Set(user_id as i64),
        })
        .on_conflict(
            OnConflict::column(lol_account::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_with_returning(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to create lol_account"))
    }
    pub async fn delete_lol_account(&self, server: String, summoner: String) -> Result<()> {
        match LolAccount::delete(lol_account::ActiveModel {
            id: NotSet,
            server: Set(server),
            summoner: Set(summoner),
            user_id: NotSet,
        })
        .exec(&self.db)
        .await
        {
            Ok(r) => {
                if r.rows_affected == 0 {
                    Err(anyhow!("No such Account"))
                } else {
                    Ok(())
                }
            }
            Err(_) => Err(anyhow!("Failed to delete task")),
        }
    }
    pub async fn get_all_lol_accounts_in_user(&self, id: u64) -> Result<Vec<lol_account::Model>> {
        let v = User::find_by_id(id as i64)
            .find_with_related(LolAccount)
            .all(&self.db)
            .await
            .map_err(|_| anyhow!("Database call failed"))?
            .first()
            .ok_or(anyhow!("No accounts in this user"))?
            .1
            .clone();
        Ok(v)
    }
    async fn get_bank_account(&self, user_id: u64) -> Result<bank_account::Model> {
        Ok(User::find_by_id(user_id as i64)
            .find_with_related(BankAccount)
            .all(&self.db)
            .await
            .map_err(|_| anyhow!("Database call failed"))?
            .first()
            .ok_or(anyhow!("User not registered in Wallace"))?
            .1
            .first()
            .ok_or(anyhow!("User has no account"))?
            .clone())
    }
    pub async fn create_bank_account(&self, user_id: u64) -> Result<bank_account::Model> {
        if self.get_bank_account(user_id).await.is_ok() {
            return Err(anyhow!("Account already open"));
        }
        let _ = self.create_user(user_id).await;
        let r = BankAccount::insert(bank_account::ActiveModel {
            id: NotSet,
            balance: NotSet,
        })
        .on_conflict(
            OnConflict::column(bank_account::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_with_returning(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to create bank_account"))?;
        User::update(user::ActiveModel {
            id: Set(user_id as i64),
            mature: NotSet,
            bank_account_id: Set(Some(r.id)),
        })
        .exec(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to update user"))?;
        Ok(r)
    }
    pub async fn delete_bank_account(&self, user_id: u64) -> Result<()> {
        User::update(user::ActiveModel {
            id: Set(user_id as i64),
            mature: NotSet,
            bank_account_id: Set(None),
        })
        .exec(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to update user"))?;
        Ok(())
    }
    pub async fn get_bank_account_balance(&self, user_id: u64) -> Result<i64> {
        let a = self.get_bank_account(user_id).await?;
        Ok(a.balance)
    }
    pub async fn has_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<()> {
        self.positive(amount)?;
        if amount > self.get_bank_account(user_id).await?.balance {
            return Err(anyhow!("Account balance too low"));
        }
        Ok(())
    }
    pub async fn add_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<i64> {
        self.positive(amount)?;
        let mut a: bank_account::ActiveModel = self.get_bank_account(user_id).await?.into();
        let new_bal = a
            .balance
            .take()
            .unwrap()
            .checked_add(amount)
            .ok_or(anyhow!("overflow"))?;
        a.balance = Set(new_bal);
        let r = a
            .update(&self.db)
            .await
            .map_err(|_| anyhow!("Failed to update balance"))?
            .balance;
        Ok(r)
    }
    pub async fn subtract_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<i64> {
        let mut a: bank_account::ActiveModel = self.get_bank_account(user_id).await?.into();
        self.has_bank_account_balance(user_id, amount).await?;
        let new_bal = a
            .balance
            .take()
            .unwrap()
            .checked_sub(amount)
            .ok_or(anyhow!("overflow"))?;
        a.balance = Set(new_bal);
        let r = a
            .update(&self.db)
            .await
            .map_err(|_| anyhow!("Failed to update balance"))?
            .balance;
        Ok(r)
    }
    pub async fn transfer_bank_account_balance(
        &self,
        from_user_id: u64,
        to_user_id: u64,
        amount: i64,
    ) -> Result<(i64, i64)> {
        if from_user_id == to_user_id {
            return Err(anyhow!("Can't transfer to self"));
        }
        let trx = self.begin().await?;
        let r1 = self
            .subtract_bank_account_balance(from_user_id, amount)
            .await?;
        let r2 = self.add_bank_account_balance(to_user_id, amount).await?;
        self.commit(trx).await?;
        Ok((r1, r2))
    }
    pub async fn create_task(
        &self,
        cron: String,
        cmd: String,
        arg: Option<String>,
        channel_id: u64,
    ) -> Result<task::Model> {
        let _ = self.create_channel(channel_id).await;
        Task::insert(task::ActiveModel {
            id: NotSet,
            cron: Set(cron),
            cmd: Set(cmd),
            arg: Set(arg),
            channel_id: Set(channel_id as i64),
        })
        .on_conflict(OnConflict::column(task::Column::Id).do_nothing().to_owned())
        .exec_with_returning(&self.db)
        .await
        .map_err(|_| anyhow!("Failed to create task"))
    }
    pub async fn delete_task(&self, id: i32) -> Result<()> {
        match Task::delete_by_id(id).exec(&self.db).await {
            Ok(r) => {
                if r.rows_affected == 0 {
                    Err(anyhow!("No such ID"))
                } else {
                    Ok(())
                }
            }
            Err(_) => Err(anyhow!("Failed to delete task")),
        }
    }
    pub async fn get_all_tasks(&self) -> Result<Vec<task::Model>> {
        Task::find()
            .all(&self.db)
            .await
            .map_err(|_| anyhow!("Failed to get tasks"))
    }
    pub async fn get_all_tasks_in_channel(&self, id: u64) -> Result<Vec<task::Model>> {
        let v = Channel::find_by_id(id as i64)
            .find_with_related(Task)
            .all(&self.db)
            .await
            .map_err(|_| anyhow!("Database call failed"))?
            .first()
            .ok_or(anyhow!("No tasks in this channel"))?
            .1
            .clone();
        Ok(v)
    }
}
