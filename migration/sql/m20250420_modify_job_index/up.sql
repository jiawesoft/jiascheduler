ALTER TABLE job
drop index uk_name,
add index idx_name (team_id, name);

ALTER TABLE job_bundle_script
drop index uk_name,
add index idx_name (team_id, name);

ALTER TABLE job_timer
drop index uk_name;

ALTER TABLE job_supervisor
drop index uk_name;
