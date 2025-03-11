import app/web
import gleam/string_tree
import simplifile
import wisp.{type Request, type Response}

pub fn handle_request(req: Request) -> Response {
  use req <- web.middleware(req)

  case wisp.path_segments(req) {
    [] -> serve_index_html()
    _ -> wisp.not_found()
  }
}

fn serve_index_html() -> Response {
  let assert Ok(priv) = wisp.priv_directory("flyradar_website")
  let assert Ok(index_html) = simplifile.read(priv <> "/index.html")
  string_tree.from_string(index_html) |> wisp.html_response(200)
}
