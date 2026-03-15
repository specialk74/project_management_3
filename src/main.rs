mod devs;
mod projects;
mod single_dev;
mod sinlge_effort;
mod workers;

use serde::{Deserialize, Serialize};

use crate::devs::Devs;
use crate::projects::Projects;
use crate::workers::Workers;
use std::fs;

#[derive(Serialize, Deserialize)]
struct App {
    workers: Workers,
    devs: Devs,
    projects: Projects,
}

impl App {
    fn save(&self, path: &str) {
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .ok()
            .unwrap();
        let _ = fs::write(path, content);
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(ron::from_str(&content)?)
    }
}

fn main() {
    let path = "workers.ron";

    let worker1 = "Pippo";
    let worker2 = "Pluto";

    let dev1 = "Dev1";
    let dev2 = "Dev2";

    let info1 = "Info1";
    let info2 = "Info2";

    let week1 = 5432;
    let week2 = 2345;

    let mut app = App {
        workers: Workers::new(),
        devs: Devs::new(),
        projects: Projects::new(),
    };

    let id_w1 = app.workers.add(worker1);
    let id_w2 = app.workers.add(worker2);

    let id_d1 = app.devs.add(dev1);
    let id_d2 = app.devs.add(dev2);

    let id_p1 = app.projects.add(info1);
    //let id_p2 = app.projects.add(info2);

    app.projects.add_dev(id_p1, id_d1);
    app.projects.add_dev_effort(id_p1, id_d1, 1000);

    app.projects.add_effort(id_p1, id_d1, week1, id_w1, 100);

    app.projects.add_effort(id_p1, id_d1, week1, id_w1, 100);
    app.projects.add_effort(id_p1, id_d1, week1, id_w2, 200);

    app.projects.add_effort(id_p1, id_d1, week2, id_w1, 300);
    app.projects.add_effort(id_p1, id_d1, week2, id_w2, 400);

    app.projects.add_effort(id_p1, id_d2, week1, id_w1, 500);
    app.projects.add_effort(id_p1, id_d2, week1, id_w2, 600);

    app.projects.add_effort(id_p1, id_d2, week2, id_w1, 700);
    app.projects.add_effort(id_p1, id_d2, week2, id_w2, 800);

    app.projects.set_note(id_p1, id_d2, week2, id_w2, "Nota 1");

    // if let Some(single) = app.single.as_mut() {
    //     single.add(week1, id_w1, 100);
    //     single.add(week1, id_w2, 200);
    //     single.set_effort(1000);
    //     single.add(week2, id_w2, 300);

    //     single.set_note(week1, id_w1, "Nota 1".to_string());
    //     single.set_note(week2, id_w1, "Nota 2".to_string());
    //     single.set_note(week2, id_w2, "Nota 3".to_string());
    // }

    app.save(path);
    let project = App::load(path).unwrap();
    let content = ron::ser::to_string_pretty(&project, ron::ser::PrettyConfig::default())
        .ok()
        .unwrap();
    println!("{}", content);
}
