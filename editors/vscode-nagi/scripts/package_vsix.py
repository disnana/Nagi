"""依存不要のローカルVSIX作成。Marketplaceへの送信は行わない。"""
import json
from pathlib import Path
from xml.sax.saxutils import escape
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED

BASE = Path(__file__).resolve().parents[1]
ROOT = BASE.parents[1]
package = json.loads((BASE / "package.json").read_text(encoding="utf-8"))
version = package["version"]
identifier = package["name"]
manifest = f'''<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="{identifier}" Version="{version}" Publisher="{package['publisher']}" />
    <DisplayName>{escape(package['displayName'])}</DisplayName>
    <Description xml:space="preserve">{escape(package['description'])}</Description>
    <Tags>nagi,language</Tags><Categories>Programming Languages,Snippets</Categories>
    <Properties>
      <Property Id="Microsoft.VisualStudio.Code.Engine" Value="{package['engines']['vscode']}" />
      <Property Id="Microsoft.VisualStudio.Code.ExtensionKind" Value="workspace" />
      <Property Id="Microsoft.VisualStudio.Code.ExecutesCode" Value="true" />
    </Properties>
  </Metadata>
  <Installation><InstallationTarget Id="Microsoft.VisualStudio.Code" /></Installation>
  <Dependencies />
  <Assets>
    <Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true" />
    <Asset Type="Microsoft.VisualStudio.Services.Content.Details" Path="extension/README.md" Addressable="true" />
    <Asset Type="Microsoft.VisualStudio.Services.Content.License" Path="extension/LICENSE.txt" Addressable="true" />
  </Assets>
</PackageManifest>'''
content_types = '''<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json" />
  <Default Extension="js" ContentType="application/javascript" />
  <Default Extension="md" ContentType="text/markdown" />
  <Default Extension="txt" ContentType="text/plain" />
  <Default Extension="vsixmanifest" ContentType="text/xml" />
</Types>'''
destination = ROOT / "build" / "distribution" / f'nagi-language-{version}.vsix'
destination.parent.mkdir(parents=True, exist_ok=True)
files = [BASE / name for name in ("package.json", "README.md", "language-configuration.json", "low-configuration.json")]
for directory in ("src", "syntaxes", "snippets"):
    files.extend(sorted((BASE / directory).glob("*")))


def write_entry(archive, name, data):
    # Exclude checkout mtimes and packaging time from release checksums.
    entry = ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    entry.compress_type = ZIP_DEFLATED
    entry.create_system = 3
    entry.external_attr = 0o100644 << 16
    archive.writestr(entry, data)


with ZipFile(destination, "w", ZIP_DEFLATED) as archive:
    write_entry(archive, "extension.vsixmanifest", manifest)
    write_entry(archive, "[Content_Types].xml", content_types)
    write_entry(archive, "extension/LICENSE.txt", (ROOT / "LICENSE").read_bytes())
    for file in files:
        write_entry(archive, "extension/" + file.relative_to(BASE).as_posix(), file.read_bytes())
print(destination)
