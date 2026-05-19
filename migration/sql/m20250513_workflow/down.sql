DROP TABLE IF EXISTS `workflow`;

DROP TABLE IF EXISTS `workflow_version`;

DROP TABLE IF EXISTS `workflow_process`;

DROP TABLE IF EXISTS `workflow_process_node`;

DROP TABLE IF EXISTS `workflow_process_node_task`;

DROP TABLE IF EXISTS `workflow_process_edge`;

ALTER TABLE
    job_schedule_history DROP COLUMN `actual_args`;

ALTER TABLE
    job_timer DROP COLUMN `job_args`;

ALTER TABLE
    job_supervisor DROP COLUMN `job_args`;

DROP TABLE IF EXISTS `workflow_timer`;

DROP TABLE IF EXISTS `job_schedule`;

ALTER TABLE
    job_schedule_history DROP COLUMN `schedule_pid`;
