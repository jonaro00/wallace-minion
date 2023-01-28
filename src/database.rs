use entity::prelude::*;
use entity::*;
use migration::OnConflict;
use sea_orm::{ActiveValue::*, DatabaseConnection, EntityTrait};

pub struct DBHandler {
    pub db: DatabaseConnection,
}

impl DBHandler {
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
    pub async fn create_task(
        &self,
        cron: String,
        cmd: String,
        arg: Option<String>,
        channel_id: u64,
    ) -> Result<task::Model, &str> {
        self.create_channel(channel_id).await?;
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
        .map_err(|_| "Fail to create task")
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
            Err(_) => Err("Fail to delete task"),
        }
    }
    pub async fn get_all_tasks(&self) -> Result<Vec<task::Model>, String> {
        let v = Channel::find()
            .find_with_related(Task)
            .all(&self.db)
            .await
            .map_err(|_| "Database call failed")?
            .into_iter()
            .flat_map(|(_, t)| t)
            .collect();
        Ok(v)
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
