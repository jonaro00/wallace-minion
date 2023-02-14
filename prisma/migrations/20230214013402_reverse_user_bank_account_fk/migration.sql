-- Delete orphan bank accounts
DELETE FROM "bank_account" b
WHERE NOT EXISTS (
  SELECT FROM "user" u
  WHERE u.bank_account_id = b.id
);

-- Add new col
ALTER TABLE "bank_account"
ADD COLUMN "user_id" BIGINT;

-- Get user ids into bank_account
UPDATE "bank_account" SET ("user_id") = (
  SELECT "id" FROM "user"
  WHERE "user"."bank_account_id" = "bank_account"."id"
);

-- DropForeignKey
ALTER TABLE "user" DROP CONSTRAINT "user_bank_account_id_fkey";

-- DropIndex
ALTER TABLE "user" DROP CONSTRAINT "user_bank_account_id_key";

-- AlterTable
ALTER TABLE "bank_account" DROP CONSTRAINT "bank_account_pkey",
DROP COLUMN "id",
ALTER COLUMN "user_id" SET NOT NULL,
ADD CONSTRAINT "bank_account_pkey" PRIMARY KEY ("user_id");

-- AlterTable
ALTER TABLE "user" DROP COLUMN "bank_account_id";

-- AddForeignKey
ALTER TABLE "bank_account" ADD CONSTRAINT "bank_account_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "user"("id") ON DELETE CASCADE ON UPDATE CASCADE;
