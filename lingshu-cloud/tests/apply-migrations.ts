import { applyD1Migrations } from "cloudflare:test";
import { env } from "cloudflare:workers";

interface TestMigration {
  name: string;
  queries: string[];
}

const testEnv = env as typeof env & { TEST_MIGRATIONS: TestMigration[] };

await applyD1Migrations(env.DB, testEnv.TEST_MIGRATIONS);
