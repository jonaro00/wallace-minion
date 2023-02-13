-- DropTable
DROP TABLE "seaql_migrations";

-- RenameForeignKey
ALTER TABLE "lol_account" RENAME CONSTRAINT "FK_lol_account_user" TO "lol_account_user_id_fkey";

-- RenameForeignKey
ALTER TABLE "rn_object" RENAME CONSTRAINT "FK_rn_object_guild" TO "rn_object_guild_id_fkey";

-- RenameForeignKey
ALTER TABLE "rn_subject" RENAME CONSTRAINT "FK_rn_subject_guild" TO "rn_subject_guild_id_fkey";

-- RenameForeignKey
ALTER TABLE "task" RENAME CONSTRAINT "FK_task_channel" TO "task_channel_id_fkey";

-- RenameForeignKey
ALTER TABLE "user" RENAME CONSTRAINT "FK_user_bank_account" TO "user_bank_account_id_fkey";
