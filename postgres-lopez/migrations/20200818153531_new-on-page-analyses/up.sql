
create table analyses (
    wave_id integer not null references waves (wave_id) on delete cascade,
    analysis_id serial primary key,
    analysis_name text not null,
    unique (wave_id, analysis_name)
);

create table analysis_results (
    wave_id integer not null references waves (wave_id) on delete cascade,
    page_id bigint not null,
    analysis_id integer not null references analyses (analysis_id)
        on delete cascade,
    result jsonb not null,
    primary key (wave_id, page_id, analysis_id)
);

create index on analysis_results (page_id, analysis_id);

create view named_analyses as (
    select
        wave_name,
        page_url,
        analysis_name,
        result
    from
        waves
            join analyses using (wave_id)
            join analysis_results using (wave_id, analysis_id)
            join pages using (page_id)
);

-- adeus, modelo antigo:
drop table on_page;

-- É a casa da mãe joana (de novo!)!
grant all on all tables in schema public to public;
grant all on all sequences in schema public to public;
grant all on all functions in schema public to public;
