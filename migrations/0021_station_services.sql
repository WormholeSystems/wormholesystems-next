-- Station services (SDE stationServices + the per-operation service sets), so the
-- navigation Find can answer "nearest system with repair / cloning / ...".
create table station_services (
    id   bigint primary key,
    name text not null
);

create table station_operation_services (
    operation_id bigint not null,
    service_id   bigint not null references station_services (id),
    primary key (operation_id, service_id)
);
