ALTER TABLE instance
ADD COLUMN `register_data` json  DEFAULT NULL COMMENT 'register data',
ADD COLUMN `sys_users` json DEFAULT NULL COMMENT 'ssh user list with config data';
