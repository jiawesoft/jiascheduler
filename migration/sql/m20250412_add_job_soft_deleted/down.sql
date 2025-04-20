alter table job
drop column is_deleted,
drop column deleted_at,
drop column deleted_by;

alter table job_timer
drop column is_deleted,
drop column deleted_at,
drop column deleted_by;

alter table job_supervisor
drop column is_deleted,
drop column deleted_at,
drop column deleted_by;

alter table job_bundle_script
drop column is_deleted,
drop column deleted_at,
drop column deleted_by;

alter table job_schedule_history
drop column is_deleted,
drop column deleted_at,
drop column deleted_by;

alter table job_running_status
drop column is_deleted,
drop column deleted_at,
drop column deleted_by;
