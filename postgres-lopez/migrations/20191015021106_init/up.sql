create table pages (
    -- use SipHash to define page_id. It will save some power.
    page_id bigint primary key,
    page_url text unique not null
);

create table waves (
    wave_id serial primary key,
    started_at timestamp not null default now(),
    wave_name text unique not null
);

-- alter table waves add column wave_name text unique;
-- update waves set wave_name = wave_id::text;
-- alter table waves alter column wave_name set not null;

create type reason_enum as enum (
    'ahref',
    'redirect',
    'canonical',
    'ext_ahref',
    'ext_ahref_no_follow'
);

create table linkage (
    wave_id integer not null references waves (wave_id),
    from_page_id bigint not null,
    to_page_id bigint not null,
    reason reason_enum not null,
    primary key (wave_id, from_page_id, to_page_id, reason)
);

create unique index on linkage (wave_id, to_page_id, from_page_id, reason);
create unique index on linkage (wave_id, from_page_id)
    where reason = 'redirect';
create unique index on linkage (wave_id, from_page_id, to_page_id)
    where reason = 'canonical';

create type search_status_enum as enum ('open', 'taken', 'closed', 'error'); 

create table "status" (
    wave_id integer not null references waves (wave_id),
    page_id bigint not null,
    status_code integer,
    search_status search_status_enum not null,
    depth smallint not null,
    constraint closing_criterion check (
        search_status != 'closed' and status_code is null
            or search_status = 'closed' and status_code is not null
    ),
    primary key (wave_id, page_id)
);

create index on "status" (status_code, wave_id);
-- This should be unique, but seems to be causing deadlocks on db:
create index on "status" (wave_id, page_id, depth)
    where search_status = 'open';
-- This should be unique, but seems to be causing deadlocks on db:
create index on "status" (wave_id, page_id, depth)
    where search_status = 'closed';

create view canonical as (
    select
        "status".wave_id,
        "status".page_id,
        coalesce(to_page_id, page_id) as canonical_page_id
    from
        "status" left join linkage
            on linkage.wave_id = "status".wave_id
                and linkage.from_page_id = "status".page_id
                and reason = 'canonical'
                and "status".search_status = 'closed'
);


create view canonical_pages as (
    select distinct
        wave_id,
        to_page_id as page_id
    from
        linkage
    where
        reason = 'canonical'
);


create view canonical_linkage as (
    select distinct
        linkage.wave_id,
        from_canonical.canonical_page_id as from_canonical_page_id,
        to_canonical.canonical_page_id as to_canonical_page_id
    from
        linkage
            join canonical as from_canonical
                on from_canonical.page_id = linkage.from_page_id
                    and from_canonical.wave_id = linkage.wave_id
            join canonical as to_canonical
                on to_canonical.page_id = linkage.to_page_id
                    and to_canonical.wave_id = linkage.wave_id
    where
        linkage.reason in ('ahref', 'redirect')
);


create table on_page (
    wave_id integer not null references waves (wave_id),
    page_id bigint not null,
    title text,
    h1 text,
    meta_description_id bigint,
    title_count smallint not null,
    h1_count smallint not null,
    meta_description_count smallint not null,
    check (title is not null or title_count = 0),
    check (h1 is not null or h1_count = 0),
    check (meta_description_id is not null or meta_description_count = 0),
    primary key (wave_id, page_id)
);

create index on on_page (wave_id, page_id) where h1 is null;
create index on on_page (wave_id, page_id) where title is null;
create index on_page_ts_title_idx on on_page using gin (to_tsvector('portuguese', title));
create index on_page_ts_h1_idx on on_page using gin (to_tsvector('portuguese', h1));

create table page_rank_canonical (
    wave_id integer not null references waves (wave_id),
    canonical_page_id bigint,
    rank float not null,
    primary key (wave_id, canonical_page_id)
);

create index on page_rank_canonical (wave_id, rank)
   ;-- include (canonical_page_id);

create view page_rank as (
    select
        wave_id,
        page_id,
        rank
    from
        page_rank_canonical join canonical using (wave_id, canonical_page_id)
);


-- É a casa da mãe joana!
grant all on all tables in schema public to public;
grant all on all sequences in schema public to public;
grant all on all functions in schema public to public;
