#[rustfmt::skip]
pub const PLAYER_SEQ_SCHEMA: &str = "
CREATE SEQUENCE IF NOT EXISTS public.player_uid_seq
    INCREMENT 1
    START 1
    MINVALUE 1
    MAXVALUE 9223372036854775807
    CACHE 1;
";

#[rustfmt::skip]
pub const PLAYER_SEQ_SCHEMA_ALTER: &str = "
ALTER SEQUENCE public.player_uid_seq
    OWNER TO postgres;
";

#[rustfmt::skip]
pub const PLAYER_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS public.player
(
    uid bigint NOT NULL DEFAULT nextval('player_uid_seq'::regclass),
    username text COLLATE pg_catalog.\"default\" NOT NULL,
    address text COLLATE pg_catalog.\"default\" NOT NULL,
    password text COLLATE pg_catalog.\"default\" NOT NULL,
    item_timer bigint NOT NULL,
    death_timer bigint NOT NULL,
    vals bigint NOT NULL,
    spawn \"location\" NOT NULL,
    pos \"location\" NOT NULL,
    email text COLLATE pg_catalog.\"default\" NOT NULL,
    sprite smallint NOT NULL,
    in_death boolean NOT NULL,
    level integer NOT NULL,
    level_exp bigint NOT NULL,
    resetcount smallint NOT NULL,
    pk boolean NOT NULL,
    data bigint[] NOT NULL,
    vital integer[] NOT NULL,
    vital_max integer[] NOT NULL,
    pass_reset_code text COLLATE pg_catalog.\"default\",
    reconnect_code text COLLATE pg_catalog.\"default\",
    access \"user_access\" NOT NULL,
    current_server text COLLATE pg_catalog.\"default\",
    created_on timestamp with time zone NOT NULL DEFAULT now(),
    CONSTRAINT player_pkey PRIMARY KEY (uid),
    CONSTRAINT email UNIQUE (email),
    CONSTRAINT username UNIQUE (username)
)

WITH (
    FILLFACTOR = 70
)
TABLESPACE pg_default;
";

#[rustfmt::skip]
pub const PLAYER_SCHEMA_ALTER: &str = "
ALTER TABLE IF EXISTS public.player
    OWNER to postgres;
";

#[rustfmt::skip]
pub const EQUIPMENT_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS public.equipment
(
    uid bigint NOT NULL,
    id smallint NOT NULL,
    num integer NOT NULL,
    val smallint NOT NULL,
    itemlevel smallint NOT NULL,
    data smallint[] NOT NULL
)

WITH (
    FILLFACTOR = 70
)
TABLESPACE pg_default;
";

#[rustfmt::skip]
pub const EQUIPMENT_SCHEMA_ALTER: &str = "
ALTER TABLE IF EXISTS public.equipment
    OWNER to postgres;
";

#[rustfmt::skip]
pub const INVENTORY_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS public.inventory
(
    uid bigint NOT NULL,
    id smallint NOT NULL,
    num integer NOT NULL,
    val smallint NOT NULL,
    itemlevel smallint NOT NULL,
    data smallint[] NOT NULL
)

WITH (
    FILLFACTOR = 70
)
TABLESPACE pg_default;
";

#[rustfmt::skip]
pub const INVENTORY_SCHEMA_ALTER: &str = "
ALTER TABLE IF EXISTS public.inventory
    OWNER to postgres;
";

#[rustfmt::skip]
pub const STORAGE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS public.storage
(
    uid bigint NOT NULL,
    id smallint NOT NULL,
    num integer NOT NULL,
    val smallint NOT NULL,
    itemlevel smallint NOT NULL,
    data smallint[] NOT NULL
)

WITH (
    FILLFACTOR = 70
)
TABLESPACE pg_default;
";

#[rustfmt::skip]
pub const STORAGE_SCHEMA_ALTER: &str = "
ALTER TABLE IF EXISTS public.storage
    OWNER to postgres;
";

#[rustfmt::skip]
pub const LOGS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS public.logs
(
    serverid smallint NOT NULL,
    userid bigint NOT NULL,
    logtype \"log_type\" NOT NULL,
    message text COLLATE pg_catalog.\"default\" NOT NULL,
    ipaddress text COLLATE pg_catalog.\"default\" NOT NULL
)

WITH (
    FILLFACTOR = 70
)
TABLESPACE pg_default;
";

#[rustfmt::skip]
pub const LOGS_SCHEMA_ALTER: &str = "
ALTER TABLE IF EXISTS public.logs
    OWNER to postgres
";
