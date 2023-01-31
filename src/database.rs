use entity::prelude::*;
use entity::*;
use migration::OnConflict;
use sea_orm::DatabaseTransaction;
#[allow(unused_imports)]
use sea_orm::{entity::*, query::*, DatabaseConnection};

pub struct DBHandler {
    pub db: DatabaseConnection,
}

impl DBHandler {
    pub async fn begin(&self) -> Result<DatabaseTransaction, &str> {
        self.db.begin().await.map_err(|_| "Database call failed")
    }
    pub async fn commit(&self, trx: DatabaseTransaction) -> Result<(), &str> {
        trx.commit().await.map_err(|_| "Database call failed")
    }
    fn positive(&self, amount: i64) -> Result<(), &str> {
        if !amount.is_positive() {
            return Err("Non-positive amount");
        }
        Ok(())
    }
    pub async fn create_guild(&self, id: u64) -> Result<guild::Model, &str> {
        Guild::insert(guild::ActiveModel { id: Set(id as i64) })
            .on_conflict(
                OnConflict::column(guild::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await
            .map_err(|_| "Failed to create guild")
    }
    pub async fn create_user(&self, id: u64) -> Result<user::Model, &str> {
        User::insert(user::ActiveModel {
            id: Set(id as i64),
            bank_account_id: Set(None),
        })
        .on_conflict(OnConflict::column(user::Column::Id).do_nothing().to_owned())
        .exec_with_returning(&self.db)
        .await
        .map_err(|_| "Failed to create user")
    }
    pub async fn get_all_users(&self) -> Result<Vec<user::Model>, &str> {
        User::find()
            .all(&self.db)
            .await
            .map_err(|_| "Failed to get users")
    }
    pub async fn create_channel(&self, id: u64) -> Result<channel::Model, &str> {
        Channel::insert(channel::ActiveModel { id: Set(id as i64) })
            .on_conflict(
                OnConflict::column(channel::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await
            .map_err(|_| "Failed to create channel")
    }
    pub async fn create_lol_account(
        &self,
        server: String,
        summoner: String,
        user_id: u64,
    ) -> Result<lol_account::Model, &str> {
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
        .map_err(|_| "Failed to create lol_account")
    }
    pub async fn delete_lol_account(&self, server: String, summoner: String) -> Result<(), &str> {
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
                    Err("No such Account")
                } else {
                    Ok(())
                }
            }
            Err(_) => Err("Failed to delete task"),
        }
    }
    pub async fn get_all_lol_accounts_in_user(
        &self,
        id: u64,
    ) -> Result<Vec<lol_account::Model>, String> {
        let v = User::find_by_id(id as i64)
            .find_with_related(LolAccount)
            .all(&self.db)
            .await
            .map_err(|_| "Database call failed")?
            .first()
            .ok_or("No accounts in this user")?
            .1
            .clone();
        Ok(v)
    }
    async fn get_bank_account(&self, user_id: u64) -> Result<bank_account::Model, &str> {
        Ok(User::find_by_id(user_id as i64)
            .find_with_related(BankAccount)
            .all(&self.db)
            .await
            .map_err(|_| "Database call failed")?
            .first()
            .ok_or("User not registered in Wallace")?
            .1
            .first()
            .ok_or("User has no account")?
            .clone())
    }
    pub async fn create_bank_account(&self, user_id: u64) -> Result<bank_account::Model, &str> {
        if self.get_bank_account(user_id).await.is_ok() {
            return Err("Account already open");
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
        .map_err(|_| "Failed to create bank_account")?;
        User::update(user::ActiveModel {
            id: Set(user_id as i64),
            bank_account_id: Set(Some(r.id)),
        })
        .exec(&self.db)
        .await
        .map_err(|_| "Failed to update user")?;
        Ok(r)
    }
    pub async fn delete_bank_account(&self, user_id: u64) -> Result<(), &str> {
        User::update(user::ActiveModel {
            id: Set(user_id as i64),
            bank_account_id: Set(None),
        })
        .exec(&self.db)
        .await
        .map_err(|_| "Failed to update user")?;
        Ok(())
    }
    pub async fn get_bank_account_balance(&self, user_id: u64) -> Result<i64, &str> {
        let a = self.get_bank_account(user_id).await?;
        Ok(a.balance)
    }
    pub async fn has_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<(), &str> {
        self.positive(amount)?;
        if amount > self.get_bank_account(user_id).await?.balance {
            return Err("Account balance too low");
        }
        Ok(())
    }
    pub async fn add_bank_account_balance(&self, user_id: u64, amount: i64) -> Result<i64, &str> {
        // let trx = self.db.begin().await.map_err(|_| "Database call failed")?;
        self.positive(amount)?;
        let mut a: bank_account::ActiveModel = self.get_bank_account(user_id).await?.into();
        let new_bal = a
            .balance
            .take()
            .unwrap()
            .checked_add(amount)
            .ok_or("overflow")?;
        a.balance = Set(new_bal);
        let r = a
            .update(&self.db)
            .await
            .map_err(|_| "Failed to update balance")?
            .balance;
        // trx.commit().await.map_err(|_| "Database call failed")?;
        Ok(r)
    }
    pub async fn subtract_bank_account_balance(
        &self,
        user_id: u64,
        amount: i64,
    ) -> Result<i64, &str> {
        // let trx = self.db.begin().await.map_err(|_| "Database call failed")?;
        let mut a: bank_account::ActiveModel = self.get_bank_account(user_id).await?.into();
        let _ = self.has_bank_account_balance(user_id, amount).await?;
        let new_bal = a
            .balance
            .take()
            .unwrap()
            .checked_sub(amount)
            .ok_or("overflow")?;
        a.balance = Set(new_bal);
        let r = a
            .update(&self.db)
            .await
            .map_err(|_| "Failed to update balance")?
            .balance;
        // trx.commit().await.map_err(|_| "Database call failed")?;
        Ok(r)
    }
    pub async fn transfer_bank_account_balance(
        &self,
        from_user_id: u64,
        to_user_id: u64,
        amount: i64,
    ) -> Result<(i64, i64), &str> {
        if from_user_id == to_user_id {
            return Err("Can't transfer to self");
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
    ) -> Result<task::Model, &str> {
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
        .map_err(|_| "Failed to create task")
    }
    pub async fn delete_task(&self, id: i32) -> Result<(), &str> {
        match Task::delete_by_id(id).exec(&self.db).await {
            Ok(r) => {
                if r.rows_affected == 0 {
                    Err("No such ID")
                } else {
                    Ok(())
                }
            }
            Err(_) => Err("Failed to delete task"),
        }
    }
    pub async fn get_all_tasks(&self) -> Result<Vec<task::Model>, &str> {
        Task::find()
            .all(&self.db)
            .await
            .map_err(|_| "Failed to get tasks")
    }
    pub async fn get_all_tasks_in_channel(&self, id: u64) -> Result<Vec<task::Model>, String> {
        let v = Channel::find_by_id(id as i64)
            .find_with_related(Task)
            .all(&self.db)
            .await
            .map_err(|_| "Database call failed")?
            .first()
            .ok_or("No tasks in this channel")?
            .1
            .clone();
        Ok(v)
    }
}
