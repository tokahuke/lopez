
drop view named_page_rank;
drop view named_page_rank_canonical;
drop view page_rank;
drop table page_rank_canonical;

create table page_rank (
    wave_id integer not null references waves (wave_id) on delete cascade,
    page_id bigint not null,
    rank float not null,
    primary key (wave_id, page_id)
);

create index on page_rank (rank desc);

create view named_page_rank as (
    select
        wave_name,
        page_url,
        rank
    from
        page_rank
            join waves on page_rank.wave_id = waves.wave_id
            join pages on page_rank.page_id = pages.page_id 
);

-- É a casa da mãe joana (de novo!)!
grant all on all tables in schema public to public;
grant all on all sequences in schema public to public;
grant all on all functions in schema public to public;
