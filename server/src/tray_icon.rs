use core::mem::MaybeUninit;
use tokio::sync::oneshot;
use trayicon::*;
use winapi::um::winuser;

pub fn start_icon(shutdown_send: oneshot::Sender<()>) {
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    enum Events {
        ClickTrayIcon,
        Exit,
    }

    let (s, r) = std::sync::mpsc::channel::<Events>();
    let icon = include_bytes!("../../icon2.ico");

    //let second_icon = Icon::from_buffer(icon2, None, None).unwrap();
    //let first_icon = Icon::from_buffer(icon, None, None).unwrap();

    // Needlessly complicated tray icon with all the whistles and bells
    let _tray_icon = TrayIconBuilder::new()
        .sender(s)
        .icon_from_buffer(icon)
        .tooltip("Event Server")
        .on_click(Events::ClickTrayIcon)
        .menu(
            MenuBuilder::new()
                .item("E&xit", Events::Exit),
        )
        .build()
        .unwrap();

    std::thread::spawn(move || {
        let shutdown = shutdown_send;
        for m in r { match m {
            Events::ClickTrayIcon => {
                match open::that("http://127.0.0.1:3001/viewer") {
                    Ok(_) => {}
                    Err(e) => {log::error!("Failed to open viewer for reason {e:?}")}
                };
            }
            Events::Exit => {
                println!("Exit");
                shutdown.send(()).unwrap();
                break;
            }
        }
        }
    });

    loop {
        unsafe {
            let mut msg = MaybeUninit::uninit();
            let bret = winuser::GetMessageA(msg.as_mut_ptr(), 0 as _, 0, 0);
            if bret > 0 {
                winuser::TranslateMessage(msg.as_ptr());
                winuser::DispatchMessageA(msg.as_ptr());
            } else {
                break;
            }
        }
    }
}
