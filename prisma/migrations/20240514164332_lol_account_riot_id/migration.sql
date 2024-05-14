DELETE FROM "lol_account";

-- AlterTable
ALTER TABLE "lol_account" DROP COLUMN "summoner",
ADD COLUMN     "name" VARCHAR(100) NOT NULL,
ADD COLUMN     "tag" VARCHAR(100) NOT NULL;
