alter table on_page add column meta_description text;

update on_page set meta_description = '#' || meta_description_id;

alter table on_page drop constraint on_page_check2;
alter table on_page add constraint on_page_check2
    check (meta_description is not null or meta_description_count = 0);

alter table on_page drop column meta_description_id;
