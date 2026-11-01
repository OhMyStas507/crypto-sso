CREATE TABLE "members" (
    "member_ref" TEXT NOT NULL,
    "request_id" TEXT NOT NULL,
    "evidence_source" TEXT NOT NULL,
    "verification_note" TEXT NOT NULL,
    "verified_at" BIGINT NOT NULL,
    "issued" BOOLEAN NOT NULL,
    PRIMARY KEY ("member_ref")
);
CREATE INDEX "index_members_by_request_id" ON "members" ("request_id");
CREATE TABLE "center_initialization" (
    "id" INTEGER NOT NULL,
    "completed" BOOLEAN NOT NULL,
    "revision" BIGINT NOT NULL,
    "draft_id" TEXT NOT NULL,
    "draft_expires_at" BIGINT NOT NULL,
    "code_hash" TEXT NOT NULL,
    "code_expires_at" BIGINT NOT NULL,
    "code_used" BOOLEAN NOT NULL,
    "draft" JSONB NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "initialization_submissions" (
    "id" TEXT NOT NULL,
    "draft_id" TEXT NOT NULL,
    "receipt_hash" TEXT NOT NULL,
    "expires_at" BIGINT NOT NULL,
    "content_digest" TEXT NOT NULL,
    "result" JSONB NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "initialization_keys" (
    "id" TEXT NOT NULL,
    "draft_id" TEXT NOT NULL,
    "organization" TEXT NOT NULL,
    "status" TEXT NOT NULL,
    "owner" TEXT NOT NULL,
    "lease_until" BIGINT NOT NULL,
    "public_json" TEXT,
    PRIMARY KEY ("id")
);
CREATE TABLE "service_settings" (
    "id" INTEGER NOT NULL,
    "revision" BIGINT NOT NULL,
    "claim_hints_revision" BIGINT NOT NULL,
    "display_name" TEXT NOT NULL,
    "description" TEXT NOT NULL,
    "contact" TEXT NOT NULL,
    "public_base_url" TEXT NOT NULL,
    "registration_enabled" BOOLEAN NOT NULL,
    "issuance_enabled" BOOLEAN NOT NULL,
    "review_ttl_seconds" BIGINT NOT NULL,
    "poll_interval_seconds" BIGINT NOT NULL,
    "updated_at" BIGINT NOT NULL,
    "claim_hints" JSONB NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "issuer_metadata" (
    "id" INTEGER NOT NULL,
    "issuer_id" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "revoked_sessions" (
    "id" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "login_challenges" (
    "id" TEXT NOT NULL,
    "raw" TEXT NOT NULL,
    "consumed" BOOLEAN NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "enrollments" (
    "request_id" TEXT NOT NULL,
    "view" JSONB NOT NULL,
    "raw_request" TEXT NOT NULL,
    "claims" JSONB NOT NULL,
    "metadata_hash" TEXT NOT NULL,
    "access_hash" TEXT NOT NULL,
    "claim_hints_revision" BIGINT,
    "hints_snapshot" JSONB,
    "application_note" TEXT,
    "decision" JSONB,
    "reviewer" TEXT,
    "reviewed_at" BIGINT,
    "blind_credential" TEXT,
    "issued_at" BIGINT,
    "generation" BIGINT NOT NULL,
    "lease_until" BIGINT NOT NULL,
    "failure_code" TEXT,
    "history" JSONB NOT NULL,
    PRIMARY KEY ("request_id")
);
CREATE TABLE "administrator_accounts" (
    "id" INTEGER NOT NULL,
    "username" TEXT NOT NULL,
    "password_hash" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "web_sessions" (
    "id" TEXT NOT NULL,
    "namespace" TEXT NOT NULL,
    "data" JSONB NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_web_sessions_by_namespace" ON "web_sessions" ("namespace");
CREATE TABLE "operation_dedup" (
    "key" TEXT NOT NULL,
    "digest" TEXT NOT NULL,
    "result" JSONB NOT NULL,
    PRIMARY KEY ("key")
);
CREATE TABLE "application_sessions" (
    "id" TEXT NOT NULL,
    "subject" TEXT NOT NULL,
    "issuer_id" TEXT NOT NULL,
    "origin" TEXT NOT NULL,
    "claims" JSONB NOT NULL,
    "expires_at" BIGINT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "claim_hint_revisions" (
    "revision" BIGINT NOT NULL,
    "hints" JSONB NOT NULL,
    PRIMARY KEY ("revision")
);
CREATE TABLE "audit_events" (
    "id" TEXT NOT NULL,
    "at" BIGINT NOT NULL,
    "actor" TEXT NOT NULL,
    "action" TEXT NOT NULL,
    "target" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
