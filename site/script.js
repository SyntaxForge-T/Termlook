/* Progressive enhancement only: the site is fully usable without this script. */
(function () {
  "use strict";

  var REPO = "SyntaxForge-T/Termlook";
  var CACHE_KEY = "termlook-release-v1";
  var CACHE_MS = 10 * 60 * 1000;

  // Copy buttons on every code block.
  document.querySelectorAll(".codeblock").forEach(function (block) {
    var code = block.querySelector("pre");
    if (!code || !navigator.clipboard) return;
    var button = document.createElement("button");
    button.type = "button";
    button.className = "copy";
    button.textContent = "Copy";
    button.setAttribute("aria-label", "Copy to clipboard");
    button.addEventListener("click", function () {
      navigator.clipboard.writeText(code.innerText.replace(/\n$/, "")).then(
        function () { flash("Copied"); },
        function () { flash("Press Ctrl+C"); }
      );
    });
    function flash(text) {
      button.textContent = text;
      setTimeout(function () { button.textContent = "Copy"; }, 1600);
    }
    block.appendChild(button);
  });

  // Point the download buttons at the newest published .deb. The releases are
  // pre-releases, which GitHub's "latest release" endpoint ignores, so list them.
  function applyRelease(info) {
    document.querySelectorAll("[data-deb-link]").forEach(function (el) {
      el.setAttribute("href", info.deb);
    });
    document.querySelectorAll("[data-release-link]").forEach(function (el) {
      el.setAttribute("href", info.page);
    });
    document.querySelectorAll("[data-release-version]").forEach(function (el) {
      el.textContent = info.version;
    });
    document.querySelectorAll("[data-release-label]").forEach(function (el) {
      el.textContent = "Download v" + info.version + " (.deb)";
    });
  }

  function parse(releases) {
    var release = (releases || []).filter(function (r) { return !r.draft; })[0];
    if (!release) return null;
    var asset = (release.assets || []).filter(function (a) { return /_amd64\.deb$/.test(a.name); })[0];
    var prefix = "https://github.com/" + REPO + "/";
    if (!asset || asset.browser_download_url.indexOf(prefix) !== 0) return null;
    return {
      version: String(release.tag_name).replace(/^v/, ""),
      deb: asset.browser_download_url,
      page: release.html_url.indexOf(prefix) === 0 ? release.html_url : prefix + "releases"
    };
  }

  if (!document.querySelector("[data-deb-link], [data-release-version]")) return;

  try {
    var cached = JSON.parse(sessionStorage.getItem(CACHE_KEY) || "null");
    if (cached && Date.now() - cached.at < CACHE_MS) { applyRelease(cached.info); return; }
  } catch (e) { /* storage can be unavailable; fall through to the network */ }

  fetch("https://api.github.com/repos/" + REPO + "/releases?per_page=5", {
    headers: { Accept: "application/vnd.github+json" }
  })
    .then(function (r) { return r.ok ? r.json() : Promise.reject(r.status); })
    .then(function (releases) {
      var info = parse(releases);
      if (!info) return;
      applyRelease(info);
      try { sessionStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), info: info })); } catch (e) { /* ignore */ }
    })
    .catch(function () { /* keep the static links to the releases page */ });
})();
