#include "mellis/MLib/ManifestBuilder.h"

namespace fl {
namespace mlib {

ManifestBuilder::ManifestBuilder(StringTableBuilder& stringTable) 
    : stringTable(stringTable) {}

void ManifestBuilder::setPackageName(const std::string& name) {
    packageName = name;
    stringTable.addString(name);
}

void ManifestBuilder::setAuthor(const std::string& authorName) {
    author = authorName;
    stringTable.addString(authorName);
}

void ManifestBuilder::setVersion(const std::string& ver) {
    version = ver;
    stringTable.addString(ver);
}

void ManifestBuilder::setLicense(const std::string& lic) {
    license = lic;
    stringTable.addString(lic);
}

void ManifestBuilder::addFeature(const std::string& feature) {
    features.push_back(feature);
    stringTable.addString(feature);
}

void ManifestBuilder::addDependency(const uint8_t uuid[16], const std::string& ver, uint64_t hash, ImportMode mode, const std::vector<std::string>& depFeatures) {
    DepInfo info;
    std::memcpy(info.moduleUUID, uuid, 16);
    info.version = ver;
    info.moduleHash = hash;
    info.importMode = mode;
    stringTable.addString(ver);

    for (const auto& f : depFeatures) {
        info.featureStrs.push_back(f);
        stringTable.addString(f);
    }

    dependencies.push_back(info);
}

void ManifestBuilder::serialize(BinaryWriter& writer) const {
    ManifestHeader header;
    header.nameStringID = stringTable.getStringOffset(packageName);
    header.authorStringID = stringTable.getStringOffset(author);
    header.versionStringID = stringTable.getStringOffset(version);
    header.licenseStringID = stringTable.getStringOffset(license);
    header.featureCount = static_cast<uint32_t>(features.size());
    header.dependencyCount = static_cast<uint32_t>(dependencies.size());

    writer.writeStruct(header);

    // Write features array
    for (const auto& feat : features) {
        writer.writeU32(stringTable.getStringOffset(feat));
    }

    // Write dependencies
    for (const auto& dep : dependencies) {
        DependencyEntry entry;
        std::memcpy(entry.moduleUUID, dep.moduleUUID, 16);
        entry.versionStringID = stringTable.getStringOffset(dep.version);
        entry.moduleHash = dep.moduleHash;
        entry.importMode = dep.importMode;
        entry.featureCount = static_cast<uint32_t>(dep.featureStrs.size());
        
        writer.writeStruct(entry);
        for (const auto& f : dep.featureStrs) {
            writer.writeU32(stringTable.getStringOffset(f));
        }
    }
}

} // namespace mlib
} // namespace fl
