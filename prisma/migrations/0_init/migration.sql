-- CreateTable
CREATE TABLE "bank_account" (
    "id" SERIAL NOT NULL,
    "balance" BIGINT NOT NULL DEFAULT 0,

    CONSTRAINT "bank_account_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "channel" (
    "id" BIGINT NOT NULL,

    CONSTRAINT "channel_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "guild" (
    "id" BIGINT NOT NULL,
    "default_name" VARCHAR(100),

    CONSTRAINT "guild_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "lol_account" (
    "id" SERIAL NOT NULL,
    "server" VARCHAR(10) NOT NULL,
    "summoner" VARCHAR(100) NOT NULL,
    "user_id" BIGINT NOT NULL,

    CONSTRAINT "lol_account_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "rn_object" (
    "guild_id" BIGINT NOT NULL,
    "value" VARCHAR(45) NOT NULL,

    CONSTRAINT "rn_object_pkey" PRIMARY KEY ("guild_id","value")
);

-- CreateTable
CREATE TABLE "rn_subject" (
    "guild_id" BIGINT NOT NULL,
    "value" VARCHAR(45) NOT NULL,

    CONSTRAINT "rn_subject_pkey" PRIMARY KEY ("guild_id","value")
);

-- CreateTable
CREATE TABLE "seaql_migrations" (
    "version" VARCHAR NOT NULL,
    "applied_at" BIGINT NOT NULL,

    CONSTRAINT "seaql_migrations_pkey" PRIMARY KEY ("version")
);

-- CreateTable
CREATE TABLE "task" (
    "id" SERIAL NOT NULL,
    "cron" VARCHAR(255) NOT NULL,
    "cmd" VARCHAR(255) NOT NULL,
    "arg" VARCHAR(255),
    "channel_id" BIGINT NOT NULL,

    CONSTRAINT "task_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "user" (
    "id" BIGINT NOT NULL,
    "bank_account_id" INTEGER,
    "mature" BOOLEAN NOT NULL DEFAULT false,

    CONSTRAINT "user_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE UNIQUE INDEX "user_bank_account_id_key" ON "user"("bank_account_id");

-- AddForeignKey
ALTER TABLE "lol_account" ADD CONSTRAINT "FK_lol_account_user" FOREIGN KEY ("user_id") REFERENCES "user"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "rn_object" ADD CONSTRAINT "FK_rn_object_guild" FOREIGN KEY ("guild_id") REFERENCES "guild"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "rn_subject" ADD CONSTRAINT "FK_rn_subject_guild" FOREIGN KEY ("guild_id") REFERENCES "guild"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "task" ADD CONSTRAINT "FK_task_channel" FOREIGN KEY ("channel_id") REFERENCES "channel"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "user" ADD CONSTRAINT "FK_user_bank_account" FOREIGN KEY ("bank_account_id") REFERENCES "bank_account"("id") ON DELETE CASCADE ON UPDATE CASCADE;

