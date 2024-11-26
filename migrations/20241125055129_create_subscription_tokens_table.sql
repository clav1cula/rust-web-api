-- Add migration script here
-- Create Subscription Tokens Table
CREATE TABLE subscription_tokens (subscription_token TEXT NOT NULL, subscriber_id UUID NOT NULL REFERENCES subscriptions (ID), PRIMARY KEY (subscription_token));
