#include "mellis/MLib/ManifestBuilder.h"

namespace fl {
namespace mlib {

ManifestBuilder::ManifestBuilder(StringTableBuilder& stringTable) 
    : stringTable(stringTable) {}

void ManifestBuilder::setPackageName(const std::string& name) {
    nameStringID = stringTable.addString(name);
}

void ManifestBuilder::setAuthor(const std::string& author) {
    authorStringID = stringTable.addString(author);
}

void ManifestBuilder::setVersion(const std::string& version) {
    versionStringID = stringTable.addString(version);
}

void ManifestBuilder::setLicense(const std::string& license) {
    licenseStringID = stringTable.addString(license);
}

void ManifestBuilder::addFeature(const std::string& feature) {
    features.push_back(stringTable.addString(feature));
}

void ManifestBuilder::addDependency(const uint8_t uuid[16], const std::string& version, uint64_t hash, ImportMode mode, const std::vector<std::string>& depFeatures) {
    DepInfo info;
    std::memcpy(info.entry.moduleUUID, uuid, 16);
    info.entry.versionStringID = stringTable.addString(version);
    info.entry.moduleHash = hash;
    info.entry.importMode = mode;
    info.entry.featureCount = static_cast<uint32_t>(depFeatures.size());

    for (const auto& f : depFeatures) {
        info.featureIDs.push_back(stringTable.addString(f));
    }

    dependencies.push_back(info);
}

void ManifestBuilder::serialize(BinaryWriter& writer) const {
    ManifestHeader header;
    header.nameStringID = nameStringID;
    header.authorStringID = authorStringID;
    header.versionStringID = versionStringID;
    header.licenseStringID = licenseStringID;
    header.featureCount = static_cast<uint32_t>(features.size());
    header.dependencyCount = static_cast<uint32_t>(dependencies.size());

    writer.writeStruct(header);

    // Write features array
    for (uint32_t featID : features) {
        writer.writeU32(featID);
    }

    // Write dependencies
    for (const auto& dep : dependencies) {
        writer.writeStruct(dep.entry);
        for (uint32_t fID : dep.featureIDs) {
            writer.writeU32(fID);
        }
    }
}

} // namespace mlib
} // namespace fl
