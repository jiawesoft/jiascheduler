ALTER TABLE job
drop index idx_name,
add UNIQUE INDEX uk_name (`name`, `team_id`);

ALTER TABLE job_bundle_script
drop index idx_name,
add UNIQUE INDEX uk_name (`name`, `team_id`);

ALTER TABLE job_timer add UNIQUE INDEX uk_name (`name`, `eid`);

ALTER TABLE job_supervisor add UNIQUE INDEX uk_name (`name`, `eid`);